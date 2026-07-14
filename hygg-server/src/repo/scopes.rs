//! Per-device document permission overrides. Each row overrides the device's
//! default access for one document id.

use sea_orm::*;

use crate::entity::device_book_scopes;
use crate::util::new_id;

#[derive(FromQueryResult, Clone, Debug)]
pub struct DeviceBookAccess {
  pub book_id: String,
  pub access: String,
}

pub async fn list_for_device(
  db: &DatabaseConnection,
  tenant_id: &str,
  device_id: &str,
) -> Result<Vec<DeviceBookAccess>, DbErr> {
  device_book_scopes::Entity::find()
    .select_only()
    .column(device_book_scopes::Column::BookId)
    .column(device_book_scopes::Column::Access)
    .filter(device_book_scopes::Column::TenantId.eq(tenant_id))
    .filter(device_book_scopes::Column::DeviceId.eq(device_id))
    .order_by_asc(device_book_scopes::Column::BookId)
    .into_model::<DeviceBookAccess>()
    .all(db)
    .await
}

/// Replace all per-document overrides for a device. An empty list means the
/// device uses its default access for every document.
pub async fn replace_for_device(
  db: &DatabaseConnection,
  tenant_id: &str,
  device_id: &str,
  overrides: &[(String, String)],
) -> Result<(), DbErr> {
  device_book_scopes::Entity::delete_many()
    .filter(device_book_scopes::Column::TenantId.eq(tenant_id))
    .filter(device_book_scopes::Column::DeviceId.eq(device_id))
    .exec(db)
    .await?;

  for (book_id, access) in overrides {
    let trimmed = book_id.trim();
    if trimmed.is_empty() {
      continue;
    }
    let access = normalized_access(access);
    device_book_scopes::ActiveModel {
      id: Set(new_id()),
      tenant_id: Set(tenant_id.to_owned()),
      device_id: Set(device_id.to_owned()),
      book_id: Set(trimmed.to_owned()),
      access: Set(access.to_owned()),
    }
    .insert(db)
    .await?;
  }
  Ok(())
}

fn normalized_access(value: &str) -> &'static str {
  match value {
    "read" => "read",
    "none" => "none",
    _ => "read_write",
  }
}
