//! Per-(user, book) highlights, keyed by their text-offset span so re-adding
//! the same span updates in place; deletions are tombstones (`deleted = 1`).
//! Conflicts resolve last-write-wins by `updated_at`.

use sea_orm::sea_query::{Alias, Expr, OnConflict};
use sea_orm::*;
use serde::Serialize;

use crate::entity::highlights;
use crate::util::new_id;

pub struct HighlightInput {
  pub book_id: String,
  pub device_id: Option<String>,
  pub start_offset: i64,
  pub end_offset: i64,
  pub op_id: String,
  pub deleted: bool,
  pub created_at: i64,
  pub updated_at: i64,
}

#[derive(FromQueryResult, Serialize, Debug, Clone)]
pub struct HighlightRow {
  pub book_id: String,
  pub start_offset: i64,
  pub end_offset: i64,
  pub deleted: i64,
  pub updated_at: i64,
}

/// Insert or update a highlight, applying last-write-wins: an existing row is
/// only overwritten when the incoming `updated_at` is at least as new.
pub async fn upsert(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
  input: &HighlightInput,
) -> Result<(), DbErr> {
  let am = highlights::ActiveModel {
    id: Set(new_id()),
    tenant_id: Set(tenant_id.to_owned()),
    user_id: Set(user_id.to_owned()),
    book_id: Set(input.book_id.clone()),
    device_id: Set(input.device_id.clone()),
    start_offset: Set(input.start_offset),
    end_offset: Set(input.end_offset),
    created_at: Set(input.created_at),
    op_id: Set(Some(input.op_id.clone())),
    deleted: Set(i64::from(input.deleted)),
    updated_at: Set(input.updated_at),
  };
  // `created_at` is deliberately absent from the update set: the span keeps the
  // timestamp of the first time it was highlighted.
  highlights::Entity::insert(am)
    .on_conflict(
      OnConflict::columns([
        highlights::Column::TenantId,
        highlights::Column::UserId,
        highlights::Column::BookId,
        highlights::Column::StartOffset,
        highlights::Column::EndOffset,
      ])
      .update_columns([
        highlights::Column::DeviceId,
        highlights::Column::OpId,
        highlights::Column::Deleted,
        highlights::Column::UpdatedAt,
      ])
      .action_and_where(
        Expr::col((Alias::new("excluded"), highlights::Column::UpdatedAt))
          .gte(Expr::col((highlights::Entity, highlights::Column::UpdatedAt))),
      )
      .to_owned(),
    )
    .exec_without_returning(db)
    .await?;
  Ok(())
}

/// Highlight rows (including tombstones) for a user changed strictly after
/// `since` (Unix millis), so another device can apply adds and removals.
pub async fn list_since(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
  since: i64,
) -> Result<Vec<HighlightRow>, DbErr> {
  highlights::Entity::find()
    .select_only()
    .column(highlights::Column::BookId)
    .column(highlights::Column::StartOffset)
    .column(highlights::Column::EndOffset)
    .column(highlights::Column::Deleted)
    .column(highlights::Column::UpdatedAt)
    .filter(highlights::Column::TenantId.eq(tenant_id))
    .filter(highlights::Column::UserId.eq(user_id))
    .filter(highlights::Column::UpdatedAt.gt(since))
    .order_by_asc(highlights::Column::UpdatedAt)
    .into_model::<HighlightRow>()
    .all(db)
    .await
}
