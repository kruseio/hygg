//! Per-(user, book) notes, keyed by the client-supplied stable note id
//! (`note_uid`, a uuid) so an edited note updates in place across devices; the
//! key is tenant-scoped so one tenant can never overwrite another's note.
//! Deletions are tombstones. Conflicts resolve last-write-wins by `updated_at`.

use sea_orm::sea_query::{Alias, Expr, OnConflict};
use sea_orm::*;
use serde::Serialize;

use crate::entity::notes;
use crate::util::new_id;

pub struct NoteInput {
  pub note_uid: String,
  pub book_id: String,
  pub device_id: Option<String>,
  pub anchor_line: Option<i64>,
  pub body: String,
  pub op_id: String,
  pub deleted: bool,
  pub created_at: i64,
  pub updated_at: i64,
}

#[derive(FromQueryResult, Serialize, Debug, Clone)]
pub struct NoteRow {
  /// The client's stable note id, so a device reconciles against its own copy.
  pub id: String,
  pub book_id: String,
  pub anchor_line: Option<i64>,
  pub body: String,
  pub deleted: i64,
  pub created_at: i64,
  pub updated_at: i64,
}

/// Insert or update a note, applying last-write-wins: an existing row is only
/// overwritten when the incoming `updated_at` is at least as new.
pub async fn upsert(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
  input: &NoteInput,
) -> Result<(), DbErr> {
  let am = notes::ActiveModel {
    id: Set(new_id()),
    tenant_id: Set(tenant_id.to_owned()),
    user_id: Set(user_id.to_owned()),
    book_id: Set(input.book_id.clone()),
    device_id: Set(input.device_id.clone()),
    note_uid: Set(input.note_uid.clone()),
    anchor_line: Set(input.anchor_line),
    body: Set(input.body.clone()),
    created_at: Set(input.created_at),
    updated_at: Set(input.updated_at),
    op_id: Set(Some(input.op_id.clone())),
    deleted: Set(i64::from(input.deleted)),
  };
  // `book_id` and `created_at` stay absent from the update set: a note keeps
  // the book and creation time it was first synced with.
  notes::Entity::insert(am)
    .on_conflict(
      OnConflict::columns([
        notes::Column::TenantId,
        notes::Column::UserId,
        notes::Column::NoteUid,
      ])
      .update_columns([
        notes::Column::DeviceId,
        notes::Column::AnchorLine,
        notes::Column::Body,
        notes::Column::UpdatedAt,
        notes::Column::OpId,
        notes::Column::Deleted,
      ])
      .action_and_where(
        Expr::col((Alias::new("excluded"), notes::Column::UpdatedAt))
          .gte(Expr::col((notes::Entity, notes::Column::UpdatedAt))),
      )
      .to_owned(),
    )
    .exec_without_returning(db)
    .await?;
  Ok(())
}

/// Note rows (including tombstones) for a user changed strictly after `since`
/// (Unix millis), so another device can apply edits and deletions.
pub async fn list_since(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
  since: i64,
) -> Result<Vec<NoteRow>, DbErr> {
  notes::Entity::find()
    .select_only()
    .column_as(notes::Column::NoteUid, "id")
    .column(notes::Column::BookId)
    .column(notes::Column::AnchorLine)
    .column(notes::Column::Body)
    .column(notes::Column::Deleted)
    .column(notes::Column::CreatedAt)
    .column(notes::Column::UpdatedAt)
    .filter(notes::Column::TenantId.eq(tenant_id))
    .filter(notes::Column::UserId.eq(user_id))
    .filter(notes::Column::UpdatedAt.gt(since))
    .order_by_asc(notes::Column::UpdatedAt)
    .into_model::<NoteRow>()
    .all(db)
    .await
}
