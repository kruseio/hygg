use sea_orm::sea_query::{Expr, IntoCondition};
use sea_orm::*;

use super::visibility;
use crate::entity::{book_blobs, books};

/// `book_blobs -> books`, re-scoped to the tenant. The generated relation joins
/// on `book_id` alone; every blob lookup must stay inside one tenant.
fn blob_book() -> RelationDef {
  book_blobs::Relation::Books.def().on_condition(|left, right| {
    Expr::col((left, book_blobs::Column::TenantId))
      .equals((right, books::Column::TenantId))
      .into_condition()
  })
}

/// `books -> book_blobs`, the same join taken from the book side.
fn book_blob() -> RelationDef {
  books::Relation::BookBlobs.def().on_condition(|left, right| {
    Expr::col((left, books::Column::TenantId))
      .equals((right, book_blobs::Column::TenantId))
      .into_condition()
  })
}

/// SUM over an empty set is NULL, which callers read as zero bytes stored.
async fn sum_byte_len(
  db: &DatabaseConnection,
  query: Select<book_blobs::Entity>,
) -> Result<i64, DbErr> {
  let total: Option<Option<i64>> = query
    .select_only()
    .column_as(book_blobs::Column::ByteLen.sum(), "total")
    .into_tuple()
    .one(db)
    .await?;
  Ok(total.flatten().unwrap_or(0))
}

/// The format tag and original file name for a document, used to re-run the
/// extraction pipeline server-side — the file name (or a synthesized
/// `document.<format>`) drives which extractor `convert_bytes` selects. Absent
/// when no book row exists for the hash.
pub async fn extract_hint(
  db: &DatabaseConnection,
  tenant_id: &str,
  content_hash: &str,
) -> Result<Option<(String, Option<String>)>, DbErr> {
  books::Entity::find()
    .select_only()
    .column(books::Column::Format)
    .column(books::Column::FileName)
    .filter(books::Column::TenantId.eq(tenant_id))
    .filter(books::Column::ContentHash.eq(content_hash))
    .into_tuple()
    .one(db)
    .await
}

/// Personal document bytes stored for this owner: the sum of blob sizes across
/// their **personal** books (organization documents belong to the org's shared
/// pool, not the uploader's). Books whose blob has been deleted count as zero.
pub async fn storage_used_by_owner(
  db: &DatabaseConnection,
  tenant_id: &str,
  owner_user_id: &str,
) -> Result<i64, DbErr> {
  sum_byte_len(
    db,
    book_blobs::Entity::find()
      .join(JoinType::InnerJoin, blob_book())
      .filter(books::Column::TenantId.eq(tenant_id))
      .filter(books::Column::OwnerUserId.eq(owner_user_id))
      .filter(books::Column::OrganizationId.is_null()),
  )
  .await
}

/// Total document bytes stored across the whole tenant — the server's storage
/// footprint, for the admin server-storage gauge and warnings.
pub async fn total_storage(
  db: &DatabaseConnection,
  tenant_id: &str,
) -> Result<i64, DbErr> {
  sum_byte_len(
    db,
    book_blobs::Entity::find()
      .filter(book_blobs::Column::TenantId.eq(tenant_id)),
  )
  .await
}

/// Document bytes stored across an organization's documents — its shared
/// storage pool, separate from every member's personal storage.
pub async fn storage_used_by_org(
  db: &DatabaseConnection,
  tenant_id: &str,
  organization_id: &str,
) -> Result<i64, DbErr> {
  sum_byte_len(
    db,
    book_blobs::Entity::find()
      .join(JoinType::InnerJoin, blob_book())
      .filter(books::Column::TenantId.eq(tenant_id))
      .filter(books::Column::OrganizationId.eq(organization_id)),
  )
  .await
}

/// Document bytes stored for one organization document (0 when no blob exists).
pub async fn size_for_org_hash(
  db: &DatabaseConnection,
  tenant_id: &str,
  organization_id: &str,
  content_hash: &str,
) -> Result<Option<i64>, DbErr> {
  book_blobs::Entity::find()
    .select_only()
    .column(book_blobs::Column::ByteLen)
    .join(JoinType::InnerJoin, blob_book())
    .filter(books::Column::TenantId.eq(tenant_id))
    .filter(books::Column::OrganizationId.eq(organization_id))
    .filter(books::Column::ContentHash.eq(content_hash))
    .into_tuple()
    .one(db)
    .await
}

/// Document bytes currently stored for one owned book (0 when no blob exists,
/// e.g. metadata-only after a document delete, or before the first upload).
pub async fn size_for_owned_hash(
  db: &DatabaseConnection,
  tenant_id: &str,
  owner_user_id: &str,
  content_hash: &str,
) -> Result<Option<i64>, DbErr> {
  book_blobs::Entity::find()
    .select_only()
    .column(book_blobs::Column::ByteLen)
    .join(JoinType::InnerJoin, blob_book())
    .filter(books::Column::TenantId.eq(tenant_id))
    .filter(books::Column::OwnerUserId.eq(owner_user_id))
    .filter(books::Column::ContentHash.eq(content_hash))
    .into_tuple()
    .one(db)
    .await
}

/// Stored document size per book for the caller's accessible library, keyed by
/// content hash. Only books that still hold bytes appear; metadata-only books
/// are absent and should be treated as zero document storage.
pub async fn blob_sizes_for_user(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
) -> Result<Vec<(String, i64)>, DbErr> {
  books::Entity::find()
    .select_only()
    .column(books::Column::ContentHash)
    .column(book_blobs::Column::ByteLen)
    .join(JoinType::InnerJoin, book_blob())
    .filter(books::Column::TenantId.eq(tenant_id))
    .filter(visibility::owned_or_org(user_id))
    .into_tuple()
    .all(db)
    .await
}

/// Delete an owned book's metadata row. Callers must delete its blob first
/// (see `blobs::delete_for_book`) since FK cascade is not enabled on SQLite.
/// Returns whether a row was removed.
pub async fn delete_for_owner(
  db: &DatabaseConnection,
  tenant_id: &str,
  owner_user_id: &str,
  content_hash: &str,
) -> Result<bool, DbErr> {
  let result = books::Entity::delete_many()
    .filter(books::Column::TenantId.eq(tenant_id))
    .filter(books::Column::OwnerUserId.eq(owner_user_id))
    .filter(books::Column::ContentHash.eq(content_hash))
    .exec(db)
    .await?;
  Ok(result.rows_affected > 0)
}
