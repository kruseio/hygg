//! Time spent reading, and the op ledger that makes sync idempotent.

use sea_orm_migration::prelude::*;
use sea_orm_migration::schema::*;

use super::identity::{Users, tenant_fk};

#[derive(DeriveIden)]
pub enum ReadingTime {
  Table,
  Id,
  TenantId,
  UserId,
  BookId,
  DeviceId,
  Seconds,
  OpId,
  UpdatedAt,
}

#[derive(DeriveIden)]
pub enum ReadingDays {
  Table,
  Id,
  TenantId,
  UserId,
  DeviceId,
  Day,
  Seconds,
  OpId,
  UpdatedAt,
}

#[derive(DeriveIden)]
pub enum AppliedOps {
  Table,
  TenantId,
  OpId,
  AppliedAt,
}

/// The `(tenant, user)` foreign key these tables carry.
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
      .table(ReadingTime::Table)
      .if_not_exists()
      .col(string(ReadingTime::Id).primary_key())
      .col(string(ReadingTime::TenantId))
      .col(string(ReadingTime::UserId))
      .col(string(ReadingTime::BookId))
      .col(string(ReadingTime::DeviceId).default(""))
      .col(big_integer(ReadingTime::Seconds).default(0))
      .col(string_null(ReadingTime::OpId))
      .col(big_integer(ReadingTime::UpdatedAt))
      .foreign_key(&mut tenant_fk(ReadingTime::Table, ReadingTime::TenantId))
      .foreign_key(&mut user_fk(ReadingTime::Table, ReadingTime::UserId))
      .index(
        Index::create()
          .name("uq_reading_time_key")
          .col(ReadingTime::TenantId)
          .col(ReadingTime::UserId)
          .col(ReadingTime::BookId)
          .col(ReadingTime::DeviceId)
          .unique(),
      )
      .to_owned(),
  )
  .await?;

  m.create_table(
    Table::create()
      .table(ReadingDays::Table)
      .if_not_exists()
      .col(string(ReadingDays::Id).primary_key())
      .col(string(ReadingDays::TenantId))
      .col(string(ReadingDays::UserId))
      .col(string(ReadingDays::DeviceId).default(""))
      .col(string(ReadingDays::Day))
      .col(big_integer(ReadingDays::Seconds).default(0))
      .col(string_null(ReadingDays::OpId))
      .col(big_integer(ReadingDays::UpdatedAt))
      .foreign_key(&mut tenant_fk(ReadingDays::Table, ReadingDays::TenantId))
      .foreign_key(&mut user_fk(ReadingDays::Table, ReadingDays::UserId))
      .index(
        Index::create()
          .name("uq_reading_days_key")
          .col(ReadingDays::TenantId)
          .col(ReadingDays::UserId)
          .col(ReadingDays::DeviceId)
          .col(ReadingDays::Day)
          .unique(),
      )
      .to_owned(),
  )
  .await?;

  m.create_table(
    Table::create()
      .table(AppliedOps::Table)
      .if_not_exists()
      .col(string(AppliedOps::TenantId))
      .col(string(AppliedOps::OpId))
      .col(big_integer(AppliedOps::AppliedAt))
      .primary_key(
        Index::create().col(AppliedOps::TenantId).col(AppliedOps::OpId),
      )
      .to_owned(),
  )
  .await?;

  Ok(())
}
