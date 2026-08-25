//! The account encryption marker: the public half of end-to-end encryption.
//!
//! One row per `(tenant, user)` recording whether encryption is on plus the
//! non-secret material a client needs to derive and verify the key (KDF, salt,
//! verifier). No key material is stored — the server cannot decrypt anything.

use sea_orm_migration::prelude::*;
use sea_orm_migration::schema::*;

use super::identity::{Users, tenant_fk};

#[derive(DeriveIden)]
pub enum EncryptionMarkers {
  Table,
  Id,
  TenantId,
  UserId,
  Enabled,
  Kdf,
  Alg,
  Salt,
  Verifier,
  CreatedAt,
  UpdatedAt,
}

pub async fn up(m: &SchemaManager<'_>) -> Result<(), DbErr> {
  m.create_table(
    Table::create()
      .table(EncryptionMarkers::Table)
      .if_not_exists()
      .col(string(EncryptionMarkers::Id).primary_key())
      .col(string(EncryptionMarkers::TenantId))
      .col(string(EncryptionMarkers::UserId))
      .col(big_integer(EncryptionMarkers::Enabled).default(0))
      .col(string(EncryptionMarkers::Kdf).default(""))
      .col(big_integer(EncryptionMarkers::Alg).default(0))
      .col(string(EncryptionMarkers::Salt).default(""))
      .col(string(EncryptionMarkers::Verifier).default(""))
      .col(big_integer(EncryptionMarkers::CreatedAt))
      .col(big_integer(EncryptionMarkers::UpdatedAt))
      .foreign_key(&mut tenant_fk(
        EncryptionMarkers::Table,
        EncryptionMarkers::TenantId,
      ))
      .foreign_key(
        &mut ForeignKey::create()
          .from(EncryptionMarkers::Table, EncryptionMarkers::UserId)
          .to(Users::Table, Users::Id)
          .on_delete(ForeignKeyAction::Cascade)
          .to_owned(),
      )
      .index(
        Index::create()
          .name("uq_encryption_markers_account")
          .col(EncryptionMarkers::TenantId)
          .col(EncryptionMarkers::UserId)
          .unique(),
      )
      .to_owned(),
  )
  .await
}
