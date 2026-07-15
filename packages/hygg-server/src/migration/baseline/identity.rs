//! Tenants, users, devices, tokens and sessions.

use sea_orm_migration::prelude::*;
use sea_orm_migration::schema::*;

pub use super::credentials::{Passkeys, RecoveryCodes};

#[derive(DeriveIden)]
pub enum Tenants {
  Table,
  Id,
  Slug,
  Name,
  Disabled,
  CreatedAt,
}

#[derive(DeriveIden)]
pub enum Users {
  Table,
  Id,
  TenantId,
  Email,
  DisplayName,
  PasswordHash,
  PasswordEnabled,
  Role,
  Disabled,
  CreatedAt,
  UpdatedAt,
}

#[derive(DeriveIden)]
pub enum Devices {
  Table,
  Id,
  TenantId,
  UserId,
  Name,
  Platform,
  DefaultAccess,
  ReadOnly,
  ProgressSyncDenied,
  Revoked,
  MachineId,
  CreatedAt,
  LastSeenAt,
}

#[derive(DeriveIden)]
pub enum ApiTokens {
  Table,
  Id,
  TenantId,
  DeviceId,
  Prefix,
  TokenHash,
  CreatedAt,
  LastUsedAt,
  ExpiresAt,
  Revoked,
}

#[derive(DeriveIden)]
pub enum DeviceBookScopes {
  Table,
  Id,
  TenantId,
  DeviceId,
  BookId,
  Access,
}

#[derive(DeriveIden)]
pub enum Sessions {
  Table,
  Id,
  TenantId,
  UserId,
  CsrfSecret,
  CreatedAt,
  ExpiresAt,
  LastUsedAt,
  Ip,
  UserAgent,
}

/// A foreign key onto `tenants(id)` that cascades — every table is scoped to a
/// tenant and goes with it.
pub fn tenant_fk(
  from: impl IntoIden + 'static,
  col: impl IntoIden,
) -> ForeignKeyCreateStatement {
  ForeignKey::create()
    .from(from, col)
    .to(Tenants::Table, Tenants::Id)
    .on_delete(ForeignKeyAction::Cascade)
    .to_owned()
}

pub async fn up(m: &SchemaManager<'_>) -> Result<(), DbErr> {
  m.create_table(
    Table::create()
      .table(Tenants::Table)
      .if_not_exists()
      .col(string(Tenants::Id).primary_key())
      .col(string_uniq(Tenants::Slug))
      .col(string(Tenants::Name))
      .col(big_integer(Tenants::Disabled).default(0))
      .col(big_integer(Tenants::CreatedAt))
      .to_owned(),
  )
  .await?;

  m.create_table(
    Table::create()
      .table(Users::Table)
      .if_not_exists()
      .col(string(Users::Id).primary_key())
      .col(string(Users::TenantId))
      .col(string(Users::Email))
      .col(string(Users::DisplayName).default(""))
      .col(string_null(Users::PasswordHash))
      .col(big_integer(Users::PasswordEnabled).default(1))
      // The two roles this server has. A deployment's own distinctions are its
      // business, not a column here.
      .col(string(Users::Role).default("user"))
      .col(big_integer(Users::Disabled).default(0))
      .col(big_integer(Users::CreatedAt))
      .col(big_integer(Users::UpdatedAt))
      .foreign_key(&mut tenant_fk(Users::Table, Users::TenantId))
      .index(
        Index::create()
          .name("uq_users_tenant_email")
          .col(Users::TenantId)
          .col(Users::Email)
          .unique(),
      )
      .to_owned(),
  )
  .await?;

  m.create_table(
    Table::create()
      .table(Devices::Table)
      .if_not_exists()
      .col(string(Devices::Id).primary_key())
      .col(string(Devices::TenantId))
      .col(string(Devices::UserId))
      .col(string(Devices::Name).default(""))
      .col(string(Devices::Platform).default(""))
      .col(string(Devices::DefaultAccess).default("read_write"))
      .col(big_integer(Devices::ReadOnly).default(0))
      .col(big_integer(Devices::ProgressSyncDenied).default(0))
      .col(big_integer(Devices::Revoked).default(0))
      .col(string_null(Devices::MachineId))
      .col(big_integer(Devices::CreatedAt))
      .col(big_integer_null(Devices::LastSeenAt))
      .foreign_key(&mut tenant_fk(Devices::Table, Devices::TenantId))
      .foreign_key(
        ForeignKey::create()
          .from(Devices::Table, Devices::UserId)
          .to(Users::Table, Users::Id)
          .on_delete(ForeignKeyAction::Cascade),
      )
      .to_owned(),
  )
  .await?;

  m.create_table(
    Table::create()
      .table(ApiTokens::Table)
      .if_not_exists()
      .col(string(ApiTokens::Id).primary_key())
      .col(string(ApiTokens::TenantId))
      .col(string(ApiTokens::DeviceId))
      .col(string_uniq(ApiTokens::Prefix))
      .col(string(ApiTokens::TokenHash))
      .col(big_integer(ApiTokens::CreatedAt))
      .col(big_integer_null(ApiTokens::LastUsedAt))
      .col(big_integer_null(ApiTokens::ExpiresAt))
      .col(big_integer(ApiTokens::Revoked).default(0))
      .foreign_key(&mut tenant_fk(ApiTokens::Table, ApiTokens::TenantId))
      .foreign_key(
        ForeignKey::create()
          .from(ApiTokens::Table, ApiTokens::DeviceId)
          .to(Devices::Table, Devices::Id)
          .on_delete(ForeignKeyAction::Cascade),
      )
      .to_owned(),
  )
  .await?;

  m.create_table(
    Table::create()
      .table(DeviceBookScopes::Table)
      .if_not_exists()
      .col(string(DeviceBookScopes::Id).primary_key())
      .col(string(DeviceBookScopes::TenantId))
      .col(string(DeviceBookScopes::DeviceId))
      .col(string(DeviceBookScopes::BookId))
      .col(string(DeviceBookScopes::Access).default("read_write"))
      .foreign_key(&mut tenant_fk(
        DeviceBookScopes::Table,
        DeviceBookScopes::TenantId,
      ))
      .foreign_key(
        ForeignKey::create()
          .from(DeviceBookScopes::Table, DeviceBookScopes::DeviceId)
          .to(Devices::Table, Devices::Id)
          .on_delete(ForeignKeyAction::Cascade),
      )
      .to_owned(),
  )
  .await?;

  m.create_table(
    Table::create()
      .table(Sessions::Table)
      .if_not_exists()
      .col(string(Sessions::Id).primary_key())
      .col(string(Sessions::TenantId))
      .col(string(Sessions::UserId))
      .col(string(Sessions::CsrfSecret))
      .col(big_integer(Sessions::CreatedAt))
      .col(big_integer(Sessions::ExpiresAt))
      .col(big_integer_null(Sessions::LastUsedAt))
      .col(string_null(Sessions::Ip))
      .col(string_null(Sessions::UserAgent))
      .foreign_key(&mut tenant_fk(Sessions::Table, Sessions::TenantId))
      .foreign_key(
        ForeignKey::create()
          .from(Sessions::Table, Sessions::UserId)
          .to(Users::Table, Users::Id)
          .on_delete(ForeignKeyAction::Cascade),
      )
      .to_owned(),
  )
  .await?;

  Ok(())
}
