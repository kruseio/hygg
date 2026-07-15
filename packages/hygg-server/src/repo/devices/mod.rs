use sea_orm::FromQueryResult;
use serde::Serialize;

mod caps;
mod queries;

pub use caps::*;
pub use queries::*;

#[derive(FromQueryResult, Clone, Debug)]
pub struct DeviceRow {
  pub id: String,
  pub tenant_id: String,
  pub user_id: String,
  pub name: String,
  pub platform: String,
  pub default_access: String,
  pub read_only: i64,
  pub progress_sync_denied: i64,
  pub revoked: i64,
  /// The machine this device is locked to, or `None` until first bound.
  pub machine_id: Option<String>,
}

/// A device as shown to its owner (no tenant/user ids).
#[derive(FromQueryResult, Serialize, Clone, Debug)]
pub struct DeviceSummary {
  pub id: String,
  pub name: String,
  pub platform: String,
  pub default_access: String,
  pub read_only: i64,
  pub progress_sync_denied: i64,
  pub revoked: i64,
  pub created_at: i64,
  pub last_seen_at: Option<i64>,
}

#[derive(FromQueryResult, Serialize, Clone, Debug)]
pub struct AdminDeviceSummary {
  pub id: String,
  pub user_id: String,
  pub email: String,
  pub name: String,
  pub platform: String,
  pub default_access: String,
  pub read_only: i64,
  pub progress_sync_denied: i64,
  pub revoked: i64,
  pub created_at: i64,
  pub last_seen_at: Option<i64>,
}
