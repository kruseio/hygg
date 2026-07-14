//! Document content storage. v1 stores bytes inline in the DB (`storage =
//! inline`); a filesystem/S3 backend can be added behind the same interface
//! later. One blob per document — a re-upload replaces the previous bytes.

use sea_orm::*;

use crate::entity::book_blobs;
use crate::util::{new_id, now_millis};

pub async fn put(
  db: &DatabaseConnection,
  tenant_id: &str,
  book_id: &str,
  bytes: &[u8],
  sha256: &str,
) -> Result<(), DbErr> {
  delete_for_book(db, tenant_id, book_id).await?;
  book_blobs::Entity::insert(book_blobs::ActiveModel {
    id: Set(new_id()),
    tenant_id: Set(tenant_id.to_owned()),
    book_id: Set(book_id.to_owned()),
    storage: Set("inline".to_owned()),
    bytes: Set(Some(bytes.to_vec())),
    byte_len: Set(bytes.len() as i64),
    sha256: Set(sha256.to_owned()),
    created_at: Set(now_millis()),
    ..Default::default()
  })
  .exec_without_returning(db)
  .await?;
  Ok(())
}

pub async fn get(
  db: &DatabaseConnection,
  tenant_id: &str,
  book_id: &str,
) -> Result<Option<Vec<u8>>, DbErr> {
  let row: Option<Option<Vec<u8>>> = book_blobs::Entity::find()
    .select_only()
    .column(book_blobs::Column::Bytes)
    .filter(book_blobs::Column::TenantId.eq(tenant_id))
    .filter(book_blobs::Column::BookId.eq(book_id))
    .into_tuple()
    .one(db)
    .await?;
  Ok(row.flatten())
}

/// Delete a document's stored bytes, keeping its book metadata row intact.
/// Returns whether a blob was present. FK cascade is not enabled on SQLite,
/// so callers that drop the book row must also call this first.
pub async fn delete_for_book(
  db: &DatabaseConnection,
  tenant_id: &str,
  book_id: &str,
) -> Result<bool, DbErr> {
  let result = book_blobs::Entity::delete_many()
    .filter(book_blobs::Column::TenantId.eq(tenant_id))
    .filter(book_blobs::Column::BookId.eq(book_id))
    .exec(db)
    .await?;
  Ok(result.rows_affected > 0)
}
