//! Organizations: membership, directories, groups and permission grants.

use sea_orm_migration::prelude::*;
use sea_orm_migration::schema::*;

use super::identity::{Users, tenant_fk};

pub use super::shares::DocumentShares;

#[derive(DeriveIden)]
pub enum Organizations {
  Table,
  Id,
  TenantId,
  Name,
  Slug,
  DefaultAccess,
  CreatedByUserId,
  CreatedAt,
  UpdatedAt,
}

#[derive(DeriveIden)]
pub enum OrganizationMembers {
  Table,
  Id,
  TenantId,
  OrganizationId,
  UserId,
  Role,
  CreatedAt,
}

#[derive(DeriveIden)]
pub enum Directories {
  Table,
  Id,
  TenantId,
  OrganizationId,
  ParentId,
  Name,
  CreatedAt,
  UpdatedAt,
}

#[derive(DeriveIden)]
pub enum OrgGroups {
  Table,
  Id,
  TenantId,
  OrganizationId,
  Name,
  CreatedAt,
  UpdatedAt,
}

#[derive(DeriveIden)]
pub enum OrgGroupMembers {
  Table,
  Id,
  TenantId,
  GroupId,
  UserId,
  CreatedAt,
}

#[derive(DeriveIden)]
pub enum DocumentPermissions {
  Table,
  Id,
  TenantId,
  OrganizationId,
  SubjectType,
  SubjectId,
  TargetType,
  TargetId,
  Access,
  CreatedAt,
  UpdatedAt,
}

fn org_fk(
  from: impl IntoIden + 'static,
  col: impl IntoIden,
) -> ForeignKeyCreateStatement {
  ForeignKey::create()
    .from(from, col)
    .to(Organizations::Table, Organizations::Id)
    .on_delete(ForeignKeyAction::Cascade)
    .to_owned()
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
      .table(Organizations::Table)
      .if_not_exists()
      .col(string(Organizations::Id).primary_key())
      .col(string(Organizations::TenantId))
      .col(string(Organizations::Name))
      .col(string(Organizations::Slug))
      .col(string(Organizations::DefaultAccess).default("read_write"))
      .col(string_null(Organizations::CreatedByUserId))
      .col(big_integer(Organizations::CreatedAt))
      .col(big_integer(Organizations::UpdatedAt))
      .foreign_key(&mut tenant_fk(
        Organizations::Table,
        Organizations::TenantId,
      ))
      .index(
        Index::create()
          .name("uq_orgs_tenant_slug")
          .col(Organizations::TenantId)
          .col(Organizations::Slug)
          .unique(),
      )
      .to_owned(),
  )
  .await?;

  m.create_table(
    Table::create()
      .table(OrganizationMembers::Table)
      .if_not_exists()
      .col(string(OrganizationMembers::Id).primary_key())
      .col(string(OrganizationMembers::TenantId))
      .col(string(OrganizationMembers::OrganizationId))
      .col(string(OrganizationMembers::UserId))
      .col(string(OrganizationMembers::Role).default("member"))
      .col(big_integer(OrganizationMembers::CreatedAt))
      .foreign_key(&mut tenant_fk(
        OrganizationMembers::Table,
        OrganizationMembers::TenantId,
      ))
      .foreign_key(&mut org_fk(
        OrganizationMembers::Table,
        OrganizationMembers::OrganizationId,
      ))
      .foreign_key(&mut user_fk(
        OrganizationMembers::Table,
        OrganizationMembers::UserId,
      ))
      .index(
        Index::create()
          .name("uq_org_members")
          .col(OrganizationMembers::TenantId)
          .col(OrganizationMembers::OrganizationId)
          .col(OrganizationMembers::UserId)
          .unique(),
      )
      .to_owned(),
  )
  .await?;

  m.create_table(
    Table::create()
      .table(Directories::Table)
      .if_not_exists()
      .col(string(Directories::Id).primary_key())
      .col(string(Directories::TenantId))
      .col(string(Directories::OrganizationId))
      .col(string_null(Directories::ParentId))
      .col(string(Directories::Name))
      .col(big_integer(Directories::CreatedAt))
      .col(big_integer(Directories::UpdatedAt))
      .foreign_key(&mut tenant_fk(Directories::Table, Directories::TenantId))
      .foreign_key(&mut org_fk(Directories::Table, Directories::OrganizationId))
      .to_owned(),
  )
  .await?;

  m.create_table(
    Table::create()
      .table(OrgGroups::Table)
      .if_not_exists()
      .col(string(OrgGroups::Id).primary_key())
      .col(string(OrgGroups::TenantId))
      .col(string(OrgGroups::OrganizationId))
      .col(string(OrgGroups::Name))
      .col(big_integer(OrgGroups::CreatedAt))
      .col(big_integer(OrgGroups::UpdatedAt))
      .foreign_key(&mut tenant_fk(OrgGroups::Table, OrgGroups::TenantId))
      .foreign_key(&mut org_fk(OrgGroups::Table, OrgGroups::OrganizationId))
      .to_owned(),
  )
  .await?;

  m.create_table(
    Table::create()
      .table(OrgGroupMembers::Table)
      .if_not_exists()
      .col(string(OrgGroupMembers::Id).primary_key())
      .col(string(OrgGroupMembers::TenantId))
      .col(string(OrgGroupMembers::GroupId))
      .col(string(OrgGroupMembers::UserId))
      .col(big_integer(OrgGroupMembers::CreatedAt))
      .foreign_key(&mut tenant_fk(
        OrgGroupMembers::Table,
        OrgGroupMembers::TenantId,
      ))
      .foreign_key(
        ForeignKey::create()
          .from(OrgGroupMembers::Table, OrgGroupMembers::GroupId)
          .to(OrgGroups::Table, OrgGroups::Id)
          .on_delete(ForeignKeyAction::Cascade),
      )
      .foreign_key(&mut user_fk(
        OrgGroupMembers::Table,
        OrgGroupMembers::UserId,
      ))
      .index(
        Index::create()
          .name("uq_org_group_members")
          .col(OrgGroupMembers::TenantId)
          .col(OrgGroupMembers::GroupId)
          .col(OrgGroupMembers::UserId)
          .unique(),
      )
      .to_owned(),
  )
  .await?;

  m.create_table(
    Table::create()
      .table(DocumentPermissions::Table)
      .if_not_exists()
      .col(string(DocumentPermissions::Id).primary_key())
      .col(string(DocumentPermissions::TenantId))
      .col(string(DocumentPermissions::OrganizationId))
      .col(string(DocumentPermissions::SubjectType))
      .col(string(DocumentPermissions::SubjectId))
      .col(string(DocumentPermissions::TargetType))
      .col(string(DocumentPermissions::TargetId))
      .col(string(DocumentPermissions::Access).default("read_write"))
      .col(big_integer(DocumentPermissions::CreatedAt))
      .col(big_integer(DocumentPermissions::UpdatedAt))
      .foreign_key(&mut tenant_fk(
        DocumentPermissions::Table,
        DocumentPermissions::TenantId,
      ))
      .foreign_key(&mut org_fk(
        DocumentPermissions::Table,
        DocumentPermissions::OrganizationId,
      ))
      .index(
        Index::create()
          .name("uq_docperms")
          .col(DocumentPermissions::TenantId)
          .col(DocumentPermissions::OrganizationId)
          .col(DocumentPermissions::SubjectType)
          .col(DocumentPermissions::SubjectId)
          .col(DocumentPermissions::TargetType)
          .col(DocumentPermissions::TargetId)
          .unique(),
      )
      .to_owned(),
  )
  .await?;

  Ok(())
}
