//! Documents shared directly between two readers, outside any organization.

use sea_orm_migration::prelude::*;
use sea_orm_migration::schema::*;

use super::identity::{Users, tenant_fk};

#[derive(DeriveIden)]
pub enum DocumentShares {
  Table,
  Id,
  TenantId,
  ContentHash,
  FromUserId,
  ToUserId,
  Access,
  Status,
  CreatedAt,
  UpdatedAt,
  RespondedAt,
}

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
      .table(DocumentShares::Table)
      .if_not_exists()
      .col(string(DocumentShares::Id).primary_key())
      .col(string(DocumentShares::TenantId))
      .col(string(DocumentShares::ContentHash))
      .col(string(DocumentShares::FromUserId))
      .col(string(DocumentShares::ToUserId))
      .col(string(DocumentShares::Access).default("read"))
      .col(string(DocumentShares::Status).default("pending"))
      .col(big_integer(DocumentShares::CreatedAt))
      .col(big_integer(DocumentShares::UpdatedAt))
      .col(big_integer_null(DocumentShares::RespondedAt))
      .foreign_key(&mut tenant_fk(
        DocumentShares::Table,
        DocumentShares::TenantId,
      ))
      .foreign_key(&mut user_fk(
        DocumentShares::Table,
        DocumentShares::FromUserId,
      ))
      .foreign_key(&mut user_fk(
        DocumentShares::Table,
        DocumentShares::ToUserId,
      ))
      .index(
        Index::create()
          .name("uq_doc_shares")
          .col(DocumentShares::TenantId)
          .col(DocumentShares::ContentHash)
          .col(DocumentShares::ToUserId)
          .unique(),
      )
      .to_owned(),
  )
  .await?;

  Ok(())
}
