//! The audit trail, in-app notifications, and the indexes that span domains.

use sea_orm_migration::prelude::*;
use sea_orm_migration::schema::*;

use super::identity::{Devices, Sessions, Users, tenant_fk};
use super::orgs::{
  Directories, DocumentPermissions, DocumentShares, OrgGroupMembers, OrgGroups,
  OrganizationMembers, Organizations,
};

#[derive(DeriveIden)]
pub enum AuditLog {
  Table,
  Id,
  TenantId,
  ActorUserId,
  ActorDeviceId,
  Action,
  TargetType,
  TargetId,
  Metadata,
  Ip,
  CreatedAt,
}

#[derive(DeriveIden)]
pub enum Notifications {
  Table,
  Id,
  TenantId,
  UserId,
  DedupeKey,
  Severity,
  Title,
  Body,
  CreatedAt,
  DismissedAt,
}

/// Create an index over `cols`, named `name`, on `table`.
async fn index(
  m: &SchemaManager<'_>,
  name: &str,
  table: impl IntoIden + 'static,
  cols: Vec<DynIden>,
) -> Result<(), DbErr> {
  let mut idx = Index::create();
  idx.name(name).table(table).if_not_exists();
  for c in cols {
    idx.col(c);
  }
  m.create_index(idx.to_owned()).await
}

pub async fn up(m: &SchemaManager<'_>) -> Result<(), DbErr> {
  m.create_table(
    Table::create()
      .table(AuditLog::Table)
      .if_not_exists()
      .col(string(AuditLog::Id).primary_key())
      .col(string(AuditLog::TenantId))
      .col(string_null(AuditLog::ActorUserId))
      .col(string_null(AuditLog::ActorDeviceId))
      .col(string(AuditLog::Action))
      .col(string_null(AuditLog::TargetType))
      .col(string_null(AuditLog::TargetId))
      .col(text_null(AuditLog::Metadata))
      .col(string_null(AuditLog::Ip))
      .col(big_integer(AuditLog::CreatedAt))
      .foreign_key(&mut tenant_fk(AuditLog::Table, AuditLog::TenantId))
      .to_owned(),
  )
  .await?;

  m.create_table(
    Table::create()
      .table(Notifications::Table)
      .if_not_exists()
      .col(string(Notifications::Id).primary_key())
      .col(string(Notifications::TenantId))
      .col(string(Notifications::UserId))
      .col(string(Notifications::DedupeKey))
      .col(string(Notifications::Severity))
      .col(string(Notifications::Title))
      .col(text(Notifications::Body))
      .col(big_integer(Notifications::CreatedAt))
      .col(big_integer_null(Notifications::DismissedAt))
      .foreign_key(&mut tenant_fk(
        Notifications::Table,
        Notifications::TenantId,
      ))
      .foreign_key(
        ForeignKey::create()
          .from(Notifications::Table, Notifications::UserId)
          .to(Users::Table, Users::Id)
          .on_delete(ForeignKeyAction::Cascade),
      )
      .index(
        Index::create()
          .name("uq_notifications_dedupe")
          .col(Notifications::TenantId)
          .col(Notifications::UserId)
          .col(Notifications::DedupeKey)
          .unique(),
      )
      .to_owned(),
  )
  .await?;

  index(
    m,
    "idx_audit_tenant_time",
    AuditLog::Table,
    vec![AuditLog::TenantId.into_iden(), AuditLog::CreatedAt.into_iden()],
  )
  .await?;
  index(
    m,
    "idx_notifications_user",
    Notifications::Table,
    vec![
      Notifications::TenantId.into_iden(),
      Notifications::UserId.into_iden(),
      Notifications::DismissedAt.into_iden(),
    ],
  )
  .await?;
  index(
    m,
    "idx_users_tenant_email",
    Users::Table,
    vec![Users::TenantId.into_iden(), Users::Email.into_iden()],
  )
  .await?;
  index(
    m,
    "idx_devices_tenant_user",
    Devices::Table,
    vec![Devices::TenantId.into_iden(), Devices::UserId.into_iden()],
  )
  .await?;
  index(
    m,
    "idx_sessions_user",
    Sessions::Table,
    vec![Sessions::TenantId.into_iden(), Sessions::UserId.into_iden()],
  )
  .await?;
  index(
    m,
    "idx_orgs_tenant_slug",
    Organizations::Table,
    vec![Organizations::TenantId.into_iden(), Organizations::Slug.into_iden()],
  )
  .await?;
  index(
    m,
    "idx_org_members_user",
    OrganizationMembers::Table,
    vec![
      OrganizationMembers::TenantId.into_iden(),
      OrganizationMembers::UserId.into_iden(),
    ],
  )
  .await?;
  index(
    m,
    "idx_org_members_org",
    OrganizationMembers::Table,
    vec![
      OrganizationMembers::TenantId.into_iden(),
      OrganizationMembers::OrganizationId.into_iden(),
    ],
  )
  .await?;
  index(
    m,
    "idx_directories_org",
    Directories::Table,
    vec![
      Directories::TenantId.into_iden(),
      Directories::OrganizationId.into_iden(),
      Directories::ParentId.into_iden(),
    ],
  )
  .await?;
  index(
    m,
    "idx_org_groups_org",
    OrgGroups::Table,
    vec![
      OrgGroups::TenantId.into_iden(),
      OrgGroups::OrganizationId.into_iden(),
    ],
  )
  .await?;
  index(
    m,
    "idx_org_group_members_user",
    OrgGroupMembers::Table,
    vec![
      OrgGroupMembers::TenantId.into_iden(),
      OrgGroupMembers::UserId.into_iden(),
    ],
  )
  .await?;
  index(
    m,
    "idx_org_group_members_group",
    OrgGroupMembers::Table,
    vec![
      OrgGroupMembers::TenantId.into_iden(),
      OrgGroupMembers::GroupId.into_iden(),
    ],
  )
  .await?;
  index(
    m,
    "idx_docperms_subject",
    DocumentPermissions::Table,
    vec![
      DocumentPermissions::TenantId.into_iden(),
      DocumentPermissions::OrganizationId.into_iden(),
      DocumentPermissions::SubjectType.into_iden(),
      DocumentPermissions::SubjectId.into_iden(),
    ],
  )
  .await?;
  index(
    m,
    "idx_docperms_target",
    DocumentPermissions::Table,
    vec![
      DocumentPermissions::TenantId.into_iden(),
      DocumentPermissions::OrganizationId.into_iden(),
      DocumentPermissions::TargetType.into_iden(),
      DocumentPermissions::TargetId.into_iden(),
    ],
  )
  .await?;
  index(
    m,
    "idx_doc_shares_from",
    DocumentShares::Table,
    vec![
      DocumentShares::TenantId.into_iden(),
      DocumentShares::FromUserId.into_iden(),
      DocumentShares::Status.into_iden(),
    ],
  )
  .await?;
  index(
    m,
    "idx_doc_shares_to",
    DocumentShares::Table,
    vec![
      DocumentShares::TenantId.into_iden(),
      DocumentShares::ToUserId.into_iden(),
      DocumentShares::Status.into_iden(),
    ],
  )
  .await?;

  Ok(())
}
