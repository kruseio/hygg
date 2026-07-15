use hygg_shared::sync::SyncMode;
use sea_orm::sea_query::OnConflict;
use sea_orm::*;
use serde::Serialize;

use crate::entity::books;
use crate::util::{new_id, now_millis};

mod placement;
mod storage;
mod visibility;

pub use placement::*;
pub use storage::*;

#[derive(FromQueryResult, Serialize, Debug, Clone)]
pub struct BookRow {
  pub owner_user_id: String,
  pub organization_id: Option<String>,
  pub directory_id: Option<String>,
  pub content_hash: String,
  pub title: String,
  pub author: String,
  pub file_name: Option<String>,
  pub format: String,
  pub size_bytes: i64,
  pub updated_at: i64,
  /// The account-wide sync ceiling (`full` | `metadata` | `off`), stored as
  /// the raw token; parse with [`SyncMode::from_token_or_default`].
  pub sync_mode: String,
}

/// The ownership/placement facts needed to resolve a caller's access to a book
/// identified by its content hash.
#[derive(FromQueryResult, Clone, Debug)]
pub struct AccessMeta {
  pub owner_user_id: String,
  pub organization_id: Option<String>,
  pub directory_id: Option<String>,
}

/// The columns behind [`BookRow`], in field order.
fn book_row_columns(query: Select<books::Entity>) -> Select<books::Entity> {
  query
    .select_only()
    .column(books::Column::OwnerUserId)
    .column(books::Column::OrganizationId)
    .column(books::Column::DirectoryId)
    .column(books::Column::ContentHash)
    .column(books::Column::Title)
    .column(books::Column::Author)
    .column(books::Column::FileName)
    .column(books::Column::Format)
    .column(books::Column::SizeBytes)
    .column(books::Column::UpdatedAt)
    .column(books::Column::SyncMode)
}

pub async fn access_meta(
  db: &DatabaseConnection,
  tenant_id: &str,
  content_hash: &str,
) -> Result<Option<AccessMeta>, DbErr> {
  books::Entity::find()
    .select_only()
    .column(books::Column::OwnerUserId)
    .column(books::Column::OrganizationId)
    .column(books::Column::DirectoryId)
    .filter(books::Column::TenantId.eq(tenant_id))
    .filter(books::Column::ContentHash.eq(content_hash))
    .into_model::<AccessMeta>()
    .one(db)
    .await
}

/// Book metadata for an upsert.
pub struct BookInput<'a> {
  pub content_hash: &'a str,
  pub title: &'a str,
  pub author: &'a str,
  pub format: &'a str,
  pub size_bytes: i64,
}

/// Upsert a book by its content hash (the client's cross-device `book_id`).
pub async fn upsert(
  db: &DatabaseConnection,
  tenant_id: &str,
  owner_user_id: &str,
  input: &BookInput<'_>,
) -> Result<(), DbErr> {
  let now = now_millis();
  // Placement and sync mode are left unset so a re-upload of an existing book
  // keeps them; on a first insert the column defaults apply.
  let book = books::ActiveModel {
    id: Set(new_id()),
    tenant_id: Set(tenant_id.to_owned()),
    owner_user_id: Set(owner_user_id.to_owned()),
    content_hash: Set(input.content_hash.to_owned()),
    title: Set(input.title.to_owned()),
    author: Set(input.author.to_owned()),
    format: Set(input.format.to_owned()),
    size_bytes: Set(input.size_bytes),
    created_at: Set(now),
    updated_at: Set(now),
    ..Default::default()
  };
  books::Entity::insert(book)
    .on_conflict(
      OnConflict::columns([
        books::Column::TenantId,
        books::Column::ContentHash,
      ])
      .update_columns([
        books::Column::Title,
        books::Column::Author,
        books::Column::Format,
        books::Column::SizeBytes,
        books::Column::UpdatedAt,
      ])
      .to_owned(),
    )
    .exec_without_returning(db)
    .await?;
  Ok(())
}

/// The account-wide sync ceiling for a document. A hash with no `books` row
/// (never registered on this server) reports [`SyncMode::Full`] — the default —
/// so a first-time upload is never blocked by a missing policy.
pub async fn sync_mode(
  db: &DatabaseConnection,
  tenant_id: &str,
  content_hash: &str,
) -> Result<SyncMode, DbErr> {
  let token: Option<String> = books::Entity::find()
    .select_only()
    .column(books::Column::SyncMode)
    .filter(books::Column::TenantId.eq(tenant_id))
    .filter(books::Column::ContentHash.eq(content_hash))
    .into_tuple()
    .one(db)
    .await?;
  Ok(token.map_or(SyncMode::Full, |t| SyncMode::from_token_or_default(&t)))
}

/// Set the account-wide sync ceiling for a document. Returns whether a row was
/// updated (false when the hash is not registered on this server yet).
pub async fn set_sync_mode(
  db: &DatabaseConnection,
  tenant_id: &str,
  content_hash: &str,
  mode: SyncMode,
) -> Result<bool, DbErr> {
  let result = books::Entity::update_many()
    .set(books::ActiveModel {
      sync_mode: Set(mode.as_str().to_owned()),
      updated_at: Set(now_millis()),
      ..Default::default()
    })
    .filter(books::Column::TenantId.eq(tenant_id))
    .filter(books::Column::ContentHash.eq(content_hash))
    .exec(db)
    .await?;
  Ok(result.rows_affected > 0)
}

pub async fn find_owner_by_hash(
  db: &DatabaseConnection,
  tenant_id: &str,
  content_hash: &str,
) -> Result<Option<String>, DbErr> {
  books::Entity::find()
    .select_only()
    .column(books::Column::OwnerUserId)
    .filter(books::Column::TenantId.eq(tenant_id))
    .filter(books::Column::ContentHash.eq(content_hash))
    .into_tuple()
    .one(db)
    .await
}

pub async fn find_id_by_hash(
  db: &DatabaseConnection,
  tenant_id: &str,
  content_hash: &str,
) -> Result<Option<String>, DbErr> {
  books::Entity::find()
    .select_only()
    .column(books::Column::Id)
    .filter(books::Column::TenantId.eq(tenant_id))
    .filter(books::Column::ContentHash.eq(content_hash))
    .into_tuple()
    .one(db)
    .await
}

pub async fn list_for_user(
  db: &DatabaseConnection,
  tenant_id: &str,
  owner_user_id: &str,
) -> Result<Vec<BookRow>, DbErr> {
  book_row_columns(books::Entity::find())
    .filter(books::Column::TenantId.eq(tenant_id))
    .filter(visibility::accessible(owner_user_id))
    .order_by_desc(books::Column::UpdatedAt)
    .into_model::<BookRow>()
    .all(db)
    .await
}

/// All documents owned by an organization (admin/owner view; not filtered by
/// the viewer's per-document permissions).
pub async fn list_for_organization(
  db: &DatabaseConnection,
  tenant_id: &str,
  organization_id: &str,
) -> Result<Vec<BookRow>, DbErr> {
  book_row_columns(books::Entity::find())
    .filter(books::Column::TenantId.eq(tenant_id))
    .filter(books::Column::OrganizationId.eq(organization_id))
    .order_by_desc(books::Column::UpdatedAt)
    .into_model::<BookRow>()
    .all(db)
    .await
}

pub async fn user_can_access_hash(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
  content_hash: &str,
) -> Result<bool, DbErr> {
  let count = books::Entity::find()
    .filter(books::Column::TenantId.eq(tenant_id))
    .filter(books::Column::ContentHash.eq(content_hash))
    .filter(visibility::accessible(user_id))
    .count(db)
    .await?;
  Ok(count > 0)
}

pub async fn user_owns_hash(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
  content_hash: &str,
) -> Result<bool, DbErr> {
  let count = books::Entity::find()
    .filter(books::Column::TenantId.eq(tenant_id))
    .filter(books::Column::OwnerUserId.eq(user_id))
    .filter(books::Column::ContentHash.eq(content_hash))
    .count(db)
    .await?;
  Ok(count > 0)
}
