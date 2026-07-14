//! Cached canonical document extraction (see migration `0021`). One row per
//! `(tenant, content hash, extractor version, justification width)`: the result
//! of running the OCR/pandoc/justify pipeline once, reused so a document is not
//! re-extracted on every client import. Derived from the retained source blob,
//! so a pipeline change (a bumped `EXTRACTOR_VERSION`) regenerates it.

use sea_orm::sea_query::OnConflict;
use sea_orm::*;

use crate::entity::book_extractions;
use crate::util::{new_id, now_millis};

/// A cached extraction result, shaped like the `/convert` response body.
#[derive(FromQueryResult, Clone, Debug)]
pub struct CachedExtraction {
  pub title: String,
  pub format: String,
  pub text: String,
}

/// The cached extraction for a document at a given pipeline version and width,
/// if one exists.
pub async fn get(
  db: &DatabaseConnection,
  tenant_id: &str,
  content_hash: &str,
  extractor_version: i64,
  col: i64,
) -> Result<Option<CachedExtraction>, DbErr> {
  book_extractions::Entity::find()
    .select_only()
    .column(book_extractions::Column::Title)
    .column(book_extractions::Column::Format)
    .column(book_extractions::Column::Text)
    .filter(book_extractions::Column::TenantId.eq(tenant_id))
    .filter(book_extractions::Column::ContentHash.eq(content_hash))
    .filter(book_extractions::Column::ExtractorVersion.eq(extractor_version))
    .filter(book_extractions::Column::Col.eq(col))
    .into_model::<CachedExtraction>()
    .one(db)
    .await
}

/// Store (or replace) the cached extraction for a document at this version and
/// width. Prunes any rows for the same document at a *different* pipeline
/// version so stale renderings do not accumulate after an `EXTRACTOR_VERSION`
/// bump. Idempotent per `(version, col)`: a re-store overwrites in place.
pub async fn put(
  db: &DatabaseConnection,
  tenant_id: &str,
  content_hash: &str,
  extractor_version: i64,
  col: i64,
  value: &CachedExtraction,
) -> Result<(), DbErr> {
  book_extractions::Entity::delete_many()
    .filter(book_extractions::Column::TenantId.eq(tenant_id))
    .filter(book_extractions::Column::ContentHash.eq(content_hash))
    .filter(book_extractions::Column::ExtractorVersion.ne(extractor_version))
    .exec(db)
    .await?;
  book_extractions::Entity::insert(book_extractions::ActiveModel {
    id: Set(new_id()),
    tenant_id: Set(tenant_id.to_owned()),
    content_hash: Set(content_hash.to_owned()),
    extractor_version: Set(extractor_version),
    col: Set(col),
    title: Set(value.title.clone()),
    format: Set(value.format.clone()),
    text: Set(value.text.clone()),
    byte_len: Set(value.text.len() as i64),
    created_at: Set(now_millis()),
  })
  .on_conflict(
    OnConflict::columns([
      book_extractions::Column::TenantId,
      book_extractions::Column::ContentHash,
      book_extractions::Column::ExtractorVersion,
      book_extractions::Column::Col,
    ])
    .update_columns([
      book_extractions::Column::Title,
      book_extractions::Column::Format,
      book_extractions::Column::Text,
      book_extractions::Column::ByteLen,
      book_extractions::Column::CreatedAt,
    ])
    .to_owned(),
  )
  .exec_without_returning(db)
  .await?;
  Ok(())
}
