//! Per-(user, book) bookmarks. Keyed by the bookmark `mark` (a single Vim
//! register letter) so re-marking the same letter updates in place; deletions
//! are tombstones (`deleted = 1`) so they propagate to other devices. Conflicts
//! resolve last-write-wins by `updated_at`.

use sea_orm::sea_query::{Alias, Expr, OnConflict};
use sea_orm::*;
use serde::Serialize;

use crate::entity::bookmarks;
use crate::util::new_id;

pub struct BookmarkInput {
  pub book_id: String,
  pub device_id: Option<String>,
  pub mark: String,
  pub line: i64,
  pub col: i64,
  pub op_id: String,
  pub deleted: bool,
  pub updated_at: i64,
}

#[derive(FromQueryResult, Serialize, Debug, Clone)]
pub struct BookmarkRow {
  pub book_id: String,
  pub mark: String,
  pub line: i64,
  pub col: i64,
  pub deleted: i64,
  pub updated_at: i64,
}

/// Insert or update a bookmark, applying last-write-wins: an existing row is
/// only overwritten when the incoming `updated_at` is at least as new.
pub async fn upsert(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
  input: &BookmarkInput,
) -> Result<(), DbErr> {
  let am = bookmarks::ActiveModel {
    id: Set(new_id()),
    tenant_id: Set(tenant_id.to_owned()),
    user_id: Set(user_id.to_owned()),
    book_id: Set(input.book_id.clone()),
    device_id: Set(input.device_id.clone()),
    mark: Set(input.mark.clone()),
    line: Set(input.line),
    col: Set(input.col),
    op_id: Set(Some(input.op_id.clone())),
    deleted: Set(i64::from(input.deleted)),
    updated_at: Set(input.updated_at),
  };
  bookmarks::Entity::insert(am)
    .on_conflict(
      OnConflict::columns([
        bookmarks::Column::TenantId,
        bookmarks::Column::UserId,
        bookmarks::Column::BookId,
        bookmarks::Column::Mark,
      ])
      .update_columns([
        bookmarks::Column::DeviceId,
        bookmarks::Column::Line,
        bookmarks::Column::Col,
        bookmarks::Column::OpId,
        bookmarks::Column::Deleted,
        bookmarks::Column::UpdatedAt,
      ])
      .action_and_where(
        Expr::col((Alias::new("excluded"), bookmarks::Column::UpdatedAt))
          .gte(Expr::col((bookmarks::Entity, bookmarks::Column::UpdatedAt))),
      )
      .to_owned(),
    )
    .exec_without_returning(db)
    .await?;
  Ok(())
}

/// Bookmark rows (including tombstones) for a user changed strictly after
/// `since` (Unix millis), so another device can apply adds and removals.
pub async fn list_since(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
  since: i64,
) -> Result<Vec<BookmarkRow>, DbErr> {
  bookmarks::Entity::find()
    .select_only()
    .column(bookmarks::Column::BookId)
    .column(bookmarks::Column::Mark)
    .column(bookmarks::Column::Line)
    .column(bookmarks::Column::Col)
    .column(bookmarks::Column::Deleted)
    .column(bookmarks::Column::UpdatedAt)
    .filter(bookmarks::Column::TenantId.eq(tenant_id))
    .filter(bookmarks::Column::UserId.eq(user_id))
    .filter(bookmarks::Column::UpdatedAt.gt(since))
    .order_by_asc(bookmarks::Column::UpdatedAt)
    .into_model::<BookmarkRow>()
    .all(db)
    .await
}
