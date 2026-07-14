use sea_orm::sea_query::{Expr, IntoCondition};
use sea_orm::*;

use super::{AdminDeviceSummary, DeviceRow, DeviceSummary};
use crate::entity::{devices, users};
use crate::util::{new_id, now_millis};

const COLUMNS: [devices::Column; 10] = [
  devices::Column::Id,
  devices::Column::TenantId,
  devices::Column::UserId,
  devices::Column::Name,
  devices::Column::Platform,
  devices::Column::DefaultAccess,
  devices::Column::ReadOnly,
  devices::Column::ProgressSyncDenied,
  devices::Column::Revoked,
  devices::Column::MachineId,
];

/// A device's owning user. Pairing on `tenant_id` as well as the user id keeps
/// the join inside one tenant rather than trusting the user id alone.
fn owner_join() -> RelationDef {
  devices::Relation::Users.def().on_condition(|left, right| {
    Expr::col((left, devices::Column::TenantId))
      .eq(Expr::col((right, users::Column::TenantId)))
      .into_condition()
  })
}

pub async fn insert(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
  name: &str,
  platform: &str,
) -> Result<String, DbErr> {
  let id = new_id();
  // The permission columns are left to their schema defaults, as the insert
  // this replaces did.
  devices::ActiveModel {
    id: Set(id.clone()),
    tenant_id: Set(tenant_id.to_owned()),
    user_id: Set(user_id.to_owned()),
    name: Set(name.to_owned()),
    platform: Set(platform.to_owned()),
    created_at: Set(now_millis()),
    ..Default::default()
  }
  .insert(db)
  .await?;
  Ok(id)
}

/// Bind a device to a machine on first use, atomically: the `machine_id IS
/// NULL` guard means only the first caller wins, so a token seen concurrently
/// from two machines binds to exactly one. Returns whether this call performed
/// the bind (`false` if it was already bound, even to the same machine).
pub async fn bind_machine_id(
  db: &DatabaseConnection,
  tenant_id: &str,
  id: &str,
  machine_id: &str,
) -> Result<bool, DbErr> {
  let result = devices::Entity::update_many()
    .col_expr(devices::Column::MachineId, Expr::value(machine_id))
    .filter(devices::Column::TenantId.eq(tenant_id))
    .filter(devices::Column::Id.eq(id))
    .filter(devices::Column::MachineId.is_null())
    .exec(db)
    .await?;
  Ok(result.rows_affected > 0)
}

pub async fn find_by_id(
  db: &DatabaseConnection,
  tenant_id: &str,
  id: &str,
) -> Result<Option<DeviceRow>, DbErr> {
  devices::Entity::find()
    .select_only()
    .columns(COLUMNS)
    .filter(devices::Column::TenantId.eq(tenant_id))
    .filter(devices::Column::Id.eq(id))
    .into_model::<DeviceRow>()
    .one(db)
    .await
}

pub async fn list_for_user(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
) -> Result<Vec<DeviceSummary>, DbErr> {
  devices::Entity::find()
    .select_only()
    .columns([
      devices::Column::Id,
      devices::Column::Name,
      devices::Column::Platform,
      devices::Column::DefaultAccess,
      devices::Column::ReadOnly,
      devices::Column::ProgressSyncDenied,
      devices::Column::Revoked,
      devices::Column::CreatedAt,
      devices::Column::LastSeenAt,
    ])
    .filter(devices::Column::TenantId.eq(tenant_id))
    .filter(devices::Column::UserId.eq(user_id))
    .order_by_desc(devices::Column::CreatedAt)
    .into_model::<DeviceSummary>()
    .all(db)
    .await
}

pub async fn list_for_tenant(
  db: &DatabaseConnection,
  tenant_id: &str,
) -> Result<Vec<AdminDeviceSummary>, DbErr> {
  devices::Entity::find()
    .join(JoinType::InnerJoin, owner_join())
    .select_only()
    .columns([devices::Column::Id, devices::Column::UserId])
    .column(users::Column::Email)
    .columns([
      devices::Column::Name,
      devices::Column::Platform,
      devices::Column::DefaultAccess,
      devices::Column::ReadOnly,
      devices::Column::ProgressSyncDenied,
      devices::Column::Revoked,
      devices::Column::CreatedAt,
      devices::Column::LastSeenAt,
    ])
    .filter(devices::Column::TenantId.eq(tenant_id))
    .order_by_desc(devices::Column::CreatedAt)
    .into_model::<AdminDeviceSummary>()
    .all(db)
    .await
}

/// Revoke one of the caller's own devices (and its tokens). Scoped by user so
/// a caller cannot revoke another user's device. Returns whether a device was
/// found and revoked.
pub async fn revoke(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
  device_id: &str,
) -> Result<bool, DbErr> {
  let result = devices::Entity::update_many()
    .col_expr(devices::Column::Revoked, Expr::value(1))
    .filter(devices::Column::TenantId.eq(tenant_id))
    .filter(devices::Column::UserId.eq(user_id))
    .filter(devices::Column::Id.eq(device_id))
    .exec(db)
    .await?;
  if result.rows_affected == 0 {
    return Ok(false);
  }
  crate::repo::tokens::revoke_for_device(db, tenant_id, device_id).await?;
  Ok(true)
}

/// Admin revoke by device id. Returns whether a device existed.
pub async fn revoke_any(
  db: &DatabaseConnection,
  tenant_id: &str,
  device_id: &str,
) -> Result<bool, DbErr> {
  let result = devices::Entity::update_many()
    .col_expr(devices::Column::Revoked, Expr::value(1))
    .filter(devices::Column::TenantId.eq(tenant_id))
    .filter(devices::Column::Id.eq(device_id))
    .exec(db)
    .await?;
  if result.rows_affected == 0 {
    return Ok(false);
  }
  crate::repo::tokens::revoke_for_device(db, tenant_id, device_id).await?;
  Ok(true)
}

pub async fn set_permissions(
  db: &DatabaseConnection,
  tenant_id: &str,
  device_id: &str,
  read_only: bool,
  progress_sync_denied: bool,
) -> Result<bool, DbErr> {
  let default_access =
    if read_only || progress_sync_denied { "read" } else { "read_write" };
  set_default_access(db, tenant_id, device_id, default_access).await
}

pub async fn set_default_access(
  db: &DatabaseConnection,
  tenant_id: &str,
  device_id: &str,
  default_access: &str,
) -> Result<bool, DbErr> {
  let default_access = normalized_access(default_access);
  let read_only = default_access != "read_write";
  let progress_sync_denied = default_access != "read_write";
  let result = devices::Entity::update_many()
    .col_expr(devices::Column::DefaultAccess, Expr::value(default_access))
    .col_expr(devices::Column::ReadOnly, Expr::value(i64::from(read_only)))
    .col_expr(
      devices::Column::ProgressSyncDenied,
      Expr::value(i64::from(progress_sync_denied)),
    )
    .filter(devices::Column::TenantId.eq(tenant_id))
    .filter(devices::Column::Id.eq(device_id))
    .exec(db)
    .await?;
  Ok(result.rows_affected > 0)
}

fn normalized_access(value: &str) -> &'static str {
  match value {
    "read" => "read",
    "none" => "none",
    _ => "read_write",
  }
}

pub async fn touch_last_seen(
  db: &DatabaseConnection,
  id: &str,
) -> Result<(), DbErr> {
  devices::Entity::update_many()
    .col_expr(devices::Column::LastSeenAt, Expr::value(now_millis()))
    .filter(devices::Column::Id.eq(id))
    .exec(db)
    .await?;
  Ok(())
}
