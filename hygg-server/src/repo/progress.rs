//! Per-(user, document) reading progress. One row per document; last-write-wins
//! by `updated_at` so a stale op (e.g. a delayed retry) never clobbers newer
//! progress.

use sea_orm::sea_query::{Alias, Expr, OnConflict};
use sea_orm::*;
use serde::Serialize;

use crate::entity::progress;
use crate::util::new_id;

pub struct ProgressInput {
  pub book_id: String,
  pub device_id: Option<String>,
  pub offset_line: i64,
  pub total_lines: i64,
  pub percentage: f64,
  pub viewport_offset: Option<i64>,
  pub cursor_y: Option<i64>,
  pub page: Option<i64>,
  pub line_in_page: Option<i64>,
  pub word_offset: Option<i64>,
  pub op_id: String,
  pub updated_at: i64,
}

#[derive(FromQueryResult, Serialize, Debug, Clone)]
pub struct ProgressRow {
  pub book_id: String,
  pub offset_line: i64,
  pub total_lines: i64,
  pub percentage: f64,
  pub viewport_offset: Option<i64>,
  pub cursor_y: Option<i64>,
  pub page: Option<i64>,
  pub line_in_page: Option<i64>,
  pub word_offset: Option<i64>,
  pub updated_at: i64,
}

/// Insert or update progress, applying last-write-wins: an existing row is only
/// overwritten when the incoming `updated_at` is at least as new.
pub async fn upsert(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
  input: &ProgressInput,
) -> Result<(), DbErr> {
  let am = progress::ActiveModel {
    id: Set(new_id()),
    tenant_id: Set(tenant_id.to_owned()),
    user_id: Set(user_id.to_owned()),
    book_id: Set(input.book_id.clone()),
    device_id: Set(input.device_id.clone()),
    offset_line: Set(input.offset_line),
    total_lines: Set(input.total_lines),
    percentage: Set(input.percentage),
    viewport_offset: Set(input.viewport_offset),
    cursor_y: Set(input.cursor_y),
    page: Set(input.page),
    line_in_page: Set(input.line_in_page),
    word_offset: Set(input.word_offset),
    op_id: Set(Some(input.op_id.clone())),
    updated_at: Set(input.updated_at),
  };
  progress::Entity::insert(am)
    .on_conflict(
      OnConflict::columns([
        progress::Column::TenantId,
        progress::Column::UserId,
        progress::Column::BookId,
      ])
      .update_columns([
        progress::Column::DeviceId,
        progress::Column::OffsetLine,
        progress::Column::TotalLines,
        progress::Column::Percentage,
        progress::Column::ViewportOffset,
        progress::Column::CursorY,
        progress::Column::Page,
        progress::Column::LineInPage,
        progress::Column::WordOffset,
        progress::Column::OpId,
        progress::Column::UpdatedAt,
      ])
      .action_and_where(
        Expr::col((Alias::new("excluded"), progress::Column::UpdatedAt))
          .gte(Expr::col((progress::Entity, progress::Column::UpdatedAt))),
      )
      .to_owned(),
    )
    .exec_without_returning(db)
    .await?;
  Ok(())
}

/// Current progress row per book for a user (one row per book; newest state).
pub async fn list_for_user(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
) -> Result<Vec<ProgressRow>, DbErr> {
  select_rows()
    .filter(progress::Column::TenantId.eq(tenant_id))
    .filter(progress::Column::UserId.eq(user_id))
    .into_model::<ProgressRow>()
    .all(db)
    .await
}

/// Progress rows for a user changed strictly after `since` (Unix millis).
pub async fn list_since(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
  since: i64,
) -> Result<Vec<ProgressRow>, DbErr> {
  select_rows()
    .filter(progress::Column::TenantId.eq(tenant_id))
    .filter(progress::Column::UserId.eq(user_id))
    .filter(progress::Column::UpdatedAt.gt(since))
    .order_by_asc(progress::Column::UpdatedAt)
    .into_model::<ProgressRow>()
    .all(db)
    .await
}

fn select_rows() -> Select<progress::Entity> {
  progress::Entity::find()
    .select_only()
    .column(progress::Column::BookId)
    .column(progress::Column::OffsetLine)
    .column(progress::Column::TotalLines)
    .column(progress::Column::Percentage)
    .column(progress::Column::ViewportOffset)
    .column(progress::Column::CursorY)
    .column(progress::Column::Page)
    .column(progress::Column::LineInPage)
    .column(progress::Column::WordOffset)
    .column(progress::Column::UpdatedAt)
}
