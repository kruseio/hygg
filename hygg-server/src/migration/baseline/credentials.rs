//! Passkeys and recovery codes: the credentials a user signs in with when a
//! password is not the answer.

use sea_orm_migration::prelude::*;
use sea_orm_migration::schema::*;

use super::identity::{Users, tenant_fk};

#[derive(DeriveIden)]
pub enum Passkeys {
  Table,
  Id,
  TenantId,
  UserId,
  CredentialId,
  PasskeyJson,
  Label,
  Disabled,
  CreatedAt,
  LastUsedAt,
}

#[derive(DeriveIden)]
pub enum RecoveryCodes {
  Table,
  Id,
  TenantId,
  UserId,
  CodeHash,
  IssuedBy,
  CreatedAt,
  ExpiresAt,
  UsedAt,
  Consumed,
}

pub async fn up(m: &SchemaManager<'_>) -> Result<(), DbErr> {
  m.create_table(
    Table::create()
      .table(Passkeys::Table)
      .if_not_exists()
      .col(string(Passkeys::Id).primary_key())
      .col(string(Passkeys::TenantId))
      .col(string(Passkeys::UserId))
      .col(string(Passkeys::CredentialId))
      .col(text(Passkeys::PasskeyJson))
      .col(string(Passkeys::Label).default(""))
      .col(big_integer(Passkeys::Disabled).default(0))
      .col(big_integer(Passkeys::CreatedAt))
      .col(big_integer_null(Passkeys::LastUsedAt))
      .foreign_key(&mut tenant_fk(Passkeys::Table, Passkeys::TenantId))
      .foreign_key(
        ForeignKey::create()
          .from(Passkeys::Table, Passkeys::UserId)
          .to(Users::Table, Users::Id)
          .on_delete(ForeignKeyAction::Cascade),
      )
      .to_owned(),
  )
  .await?;

  m.create_table(
    Table::create()
      .table(RecoveryCodes::Table)
      .if_not_exists()
      .col(string(RecoveryCodes::Id).primary_key())
      .col(string(RecoveryCodes::TenantId))
      .col(string(RecoveryCodes::UserId))
      .col(string(RecoveryCodes::CodeHash))
      .col(string_null(RecoveryCodes::IssuedBy))
      .col(big_integer(RecoveryCodes::CreatedAt))
      .col(big_integer(RecoveryCodes::ExpiresAt))
      .col(big_integer_null(RecoveryCodes::UsedAt))
      .col(big_integer(RecoveryCodes::Consumed).default(0))
      .foreign_key(&mut tenant_fk(
        RecoveryCodes::Table,
        RecoveryCodes::TenantId,
      ))
      .foreign_key(
        ForeignKey::create()
          .from(RecoveryCodes::Table, RecoveryCodes::UserId)
          .to(Users::Table, Users::Id)
          .on_delete(ForeignKeyAction::Cascade),
      )
      .to_owned(),
  )
  .await?;

  Ok(())
}
