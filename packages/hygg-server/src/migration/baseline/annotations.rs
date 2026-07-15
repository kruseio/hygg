//! Reading position and the marks a reader leaves on a document.

use sea_orm_migration::prelude::*;
use sea_orm_migration::schema::*;

use super::identity::{Users, tenant_fk};

pub use super::reading::{AppliedOps, ReadingDays, ReadingTime};

#[derive(DeriveIden)]
pub enum Progress {
  Table,
  Id,
  TenantId,
  UserId,
  BookId,
  DeviceId,
  OffsetLine,
  TotalLines,
  Percentage,
  ViewportOffset,
  CursorY,
  Page,
  LineInPage,
  WordOffset,
  OpId,
  UpdatedAt,
}

#[derive(DeriveIden)]
pub enum Bookmarks {
  Table,
  Id,
  TenantId,
  UserId,
  BookId,
  DeviceId,
  Mark,
  Line,
  Col,
  OpId,
  Deleted,
  UpdatedAt,
}

#[derive(DeriveIden)]
pub enum Highlights {
  Table,
  Id,
  TenantId,
  UserId,
  BookId,
  DeviceId,
  StartOffset,
  EndOffset,
  CreatedAt,
  OpId,
  Deleted,
  UpdatedAt,
}

#[derive(DeriveIden)]
pub enum Notes {
  Table,
  Id,
  TenantId,
  UserId,
  BookId,
  DeviceId,
  NoteUid,
  AnchorLine,
  Body,
  CreatedAt,
  UpdatedAt,
  OpId,
  Deleted,
}

/// The `(tenant, user)` foreign keys every annotation table carries.
fn user_fk(
  from: impl IntoIden + 'static,
  col: impl IntoIden,
) -> ForeignKeyCreateStatement {
  ForeignKey::create()
    .from(from, col)
    .to(Users::Table, Users::Id)
    .on_delete(ForeignKeyAction::Cascade)
    .to_owned()
}

pub async fn up(m: &SchemaManager<'_>) -> Result<(), DbErr> {
  m.create_table(
    Table::create()
      .table(Progress::Table)
      .if_not_exists()
      .col(string(Progress::Id).primary_key())
      .col(string(Progress::TenantId))
      .col(string(Progress::UserId))
      .col(string(Progress::BookId))
      .col(string_null(Progress::DeviceId))
      .col(big_integer(Progress::OffsetLine).default(0))
      .col(big_integer(Progress::TotalLines).default(0))
      .col(double(Progress::Percentage).default(0.0))
      .col(big_integer_null(Progress::ViewportOffset))
      .col(big_integer_null(Progress::CursorY))
      .col(big_integer_null(Progress::Page))
      .col(big_integer_null(Progress::LineInPage))
      .col(big_integer_null(Progress::WordOffset))
      .col(string_null(Progress::OpId))
      .col(big_integer(Progress::UpdatedAt))
      .foreign_key(&mut tenant_fk(Progress::Table, Progress::TenantId))
      .foreign_key(&mut user_fk(Progress::Table, Progress::UserId))
      .index(
        Index::create()
          .name("uq_progress_key")
          .col(Progress::TenantId)
          .col(Progress::UserId)
          .col(Progress::BookId)
          .unique(),
      )
      .to_owned(),
  )
  .await?;

  m.create_table(
    Table::create()
      .table(Bookmarks::Table)
      .if_not_exists()
      .col(string(Bookmarks::Id).primary_key())
      .col(string(Bookmarks::TenantId))
      .col(string(Bookmarks::UserId))
      .col(string(Bookmarks::BookId))
      .col(string_null(Bookmarks::DeviceId))
      .col(string(Bookmarks::Mark))
      .col(big_integer(Bookmarks::Line).default(0))
      .col(big_integer(Bookmarks::Col).default(0))
      .col(string_null(Bookmarks::OpId))
      .col(big_integer(Bookmarks::Deleted).default(0))
      .col(big_integer(Bookmarks::UpdatedAt))
      .foreign_key(&mut tenant_fk(Bookmarks::Table, Bookmarks::TenantId))
      .foreign_key(&mut user_fk(Bookmarks::Table, Bookmarks::UserId))
      .index(
        Index::create()
          .name("uq_bookmarks_key")
          .col(Bookmarks::TenantId)
          .col(Bookmarks::UserId)
          .col(Bookmarks::BookId)
          .col(Bookmarks::Mark)
          .unique(),
      )
      .to_owned(),
  )
  .await?;

  m.create_table(
    Table::create()
      .table(Highlights::Table)
      .if_not_exists()
      .col(string(Highlights::Id).primary_key())
      .col(string(Highlights::TenantId))
      .col(string(Highlights::UserId))
      .col(string(Highlights::BookId))
      .col(string_null(Highlights::DeviceId))
      .col(big_integer(Highlights::StartOffset).default(0))
      .col(big_integer(Highlights::EndOffset).default(0))
      .col(big_integer(Highlights::CreatedAt))
      .col(string_null(Highlights::OpId))
      .col(big_integer(Highlights::Deleted).default(0))
      .col(big_integer(Highlights::UpdatedAt))
      .foreign_key(&mut tenant_fk(Highlights::Table, Highlights::TenantId))
      .foreign_key(&mut user_fk(Highlights::Table, Highlights::UserId))
      .index(
        Index::create()
          .name("uq_highlights_key")
          .col(Highlights::TenantId)
          .col(Highlights::UserId)
          .col(Highlights::BookId)
          .col(Highlights::StartOffset)
          .col(Highlights::EndOffset)
          .unique(),
      )
      .to_owned(),
  )
  .await?;

  m.create_table(
    Table::create()
      .table(Notes::Table)
      .if_not_exists()
      .col(string(Notes::Id).primary_key())
      .col(string(Notes::TenantId))
      .col(string(Notes::UserId))
      .col(string(Notes::BookId))
      .col(string_null(Notes::DeviceId))
      .col(string(Notes::NoteUid).default(""))
      .col(big_integer_null(Notes::AnchorLine))
      .col(text(Notes::Body).default(""))
      .col(big_integer(Notes::CreatedAt))
      .col(big_integer(Notes::UpdatedAt))
      .col(string_null(Notes::OpId))
      .col(big_integer(Notes::Deleted).default(0))
      .foreign_key(&mut tenant_fk(Notes::Table, Notes::TenantId))
      .foreign_key(&mut user_fk(Notes::Table, Notes::UserId))
      .index(
        Index::create()
          .name("uq_notes_uid")
          .col(Notes::TenantId)
          .col(Notes::UserId)
          .col(Notes::NoteUid)
          .unique(),
      )
      .to_owned(),
  )
  .await?;

  // The idempotency ledger: an op id seen twice is applied once.
  for (name, table, cols) in [
    (
      "idx_progress_key",
      Progress::Table.into_iden(),
      vec![
        Progress::TenantId.into_iden(),
        Progress::UserId.into_iden(),
        Progress::BookId.into_iden(),
      ],
    ),
    (
      "idx_bookmarks_key",
      Bookmarks::Table.into_iden(),
      vec![
        Bookmarks::TenantId.into_iden(),
        Bookmarks::UserId.into_iden(),
        Bookmarks::BookId.into_iden(),
      ],
    ),
    (
      "idx_highlights_key",
      Highlights::Table.into_iden(),
      vec![
        Highlights::TenantId.into_iden(),
        Highlights::UserId.into_iden(),
        Highlights::BookId.into_iden(),
      ],
    ),
    (
      "idx_notes_key",
      Notes::Table.into_iden(),
      vec![
        Notes::TenantId.into_iden(),
        Notes::UserId.into_iden(),
        Notes::BookId.into_iden(),
      ],
    ),
  ] {
    let mut idx = Index::create();
    idx.name(name).table(table).if_not_exists();
    for c in cols {
      idx.col(c);
    }
    m.create_index(idx.to_owned()).await?;
  }

  Ok(())
}
