use sea_orm::sea_query::{Func, SimpleExpr};
use sea_orm::*;

use super::activity::{activity, resource_metrics};
use super::breakdown::{client_os, role_breakdown};
use super::queries::{count, sum};
use super::types::DashboardMetrics;
use crate::entity::{
  applied_ops, book_blobs, books, devices, organization_members, organizations,
  passkeys, recovery_codes, sessions, users,
};

fn text_len(column: books::Column) -> SimpleExpr {
  Func::char_length(column.into_simple_expr()).into()
}

pub async fn load(
  db: &DatabaseConnection,
  tenant_id: &str,
  since: i64,
  now: i64,
) -> Result<DashboardMetrics, DbErr> {
  let all_users =
    || users::Entity::find().filter(users::Column::TenantId.eq(tenant_id));
  let all_devices =
    || devices::Entity::find().filter(devices::Column::TenantId.eq(tenant_id));
  let all_books =
    || books::Entity::find().filter(books::Column::TenantId.eq(tenant_id));
  let all_orgs = || {
    organizations::Entity::find()
      .filter(organizations::Column::TenantId.eq(tenant_id))
  };
  let users_total = count(db, all_users()).await?;
  let users_new =
    count(db, all_users().filter(users::Column::CreatedAt.gte(since))).await?;
  let users_admin = count(
    db,
    all_users()
      .filter(users::Column::Role.eq("admin"))
      .filter(users::Column::Disabled.eq(0)),
  )
  .await?;
  let users_disabled =
    count(db, all_users().filter(users::Column::Disabled.eq(1))).await?;
  let devices_total = count(db, all_devices()).await?;
  let devices_active =
    count(db, all_devices().filter(devices::Column::Revoked.eq(0))).await?;
  let devices_seen = count(
    db,
    all_devices()
      .filter(devices::Column::Revoked.eq(0))
      .filter(devices::Column::LastSeenAt.gte(since)),
  )
  .await?;
  let devices_revoked =
    count(db, all_devices().filter(devices::Column::Revoked.eq(1))).await?;
  let documents_total = count(db, all_books()).await?;
  let documents_new =
    count(db, all_books().filter(books::Column::CreatedAt.gte(since))).await?;
  let organization_documents =
    count(db, all_books().filter(books::Column::OrganizationId.is_not_null()))
      .await?;
  let storage_bytes = sum(
    db,
    book_blobs::Entity::find()
      .filter(book_blobs::Column::TenantId.eq(tenant_id)),
    book_blobs::Column::ByteLen.into_simple_expr(),
  )
  .await?;
  // Metadata storage estimate: the variable-length text columns plus a fixed
  // per-row allowance for ids, timestamps and index entries. Mirrors the
  // per-book estimate the user dashboard shows (see web::metadata_bytes).
  const METADATA_ROW_OVERHEAD: i64 = 160;
  let metadata_text_bytes = sum(
    db,
    all_books(),
    text_len(books::Column::ContentHash)
      .add(text_len(books::Column::Title))
      .add(text_len(books::Column::Author))
      .add(text_len(books::Column::Format)),
  )
  .await?;
  let metadata_bytes =
    metadata_text_bytes + METADATA_ROW_OVERHEAD * documents_total;
  let organizations_total = count(db, all_orgs()).await?;
  let organizations_new =
    count(db, all_orgs().filter(organizations::Column::CreatedAt.gte(since)))
      .await?;
  let organization_members = count(
    db,
    organization_members::Entity::find()
      .filter(organization_members::Column::TenantId.eq(tenant_id)),
  )
  .await?;
  let sync_ops = count(
    db,
    applied_ops::Entity::find()
      .filter(applied_ops::Column::TenantId.eq(tenant_id))
      .filter(applied_ops::Column::AppliedAt.gte(since)),
  )
  .await?;
  let active_sessions = count(
    db,
    sessions::Entity::find()
      .filter(sessions::Column::TenantId.eq(tenant_id))
      .filter(sessions::Column::ExpiresAt.gt(now)),
  )
  .await?;
  let passkeys_active = count(
    db,
    passkeys::Entity::find()
      .filter(passkeys::Column::TenantId.eq(tenant_id))
      .filter(passkeys::Column::Disabled.eq(0)),
  )
  .await?;
  let recovery_active = count(
    db,
    recovery_codes::Entity::find()
      .filter(recovery_codes::Column::TenantId.eq(tenant_id))
      .filter(recovery_codes::Column::Consumed.eq(0))
      .filter(recovery_codes::Column::ExpiresAt.gt(now)),
  )
  .await?;
  Ok(DashboardMetrics {
    users_total,
    users_new,
    users_admin,
    users_disabled,
    devices_total,
    devices_active,
    devices_seen,
    devices_revoked,
    documents_total,
    documents_new,
    organization_documents,
    storage_bytes,
    metadata_bytes,
    organizations_total,
    organizations_new,
    organization_members,
    sync_ops,
    active_sessions,
    passkeys_active,
    recovery_active,
    role_breakdown: role_breakdown(db, tenant_id).await?,
    client_os: client_os(db, tenant_id, since).await?,
    activity: activity(db, tenant_id, since).await?,
    resource_metrics: resource_metrics(db, tenant_id, since).await?,
  })
}
