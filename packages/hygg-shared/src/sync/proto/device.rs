//! Device-registration and identity shapes.

use serde::{Deserialize, Serialize};

/// `POST /api/v1/devices/register` request body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterDeviceRequest {
  pub email: String,
  pub password: String,
  #[serde(default)]
  pub device_name: String,
  #[serde(default)]
  pub platform: String,
  /// Stable machine id to bind the new device to on creation. Optional and
  /// defaulted so older clients (and the machine-id-less test path) still
  /// deserialize; when present the device is locked to this machine.
  #[serde(default)]
  pub machine_id: Option<String>,
}

/// `POST /api/v1/devices/register` response body. The token is shown once.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterDeviceResponse {
  pub device_id: String,
  pub token: String,
  pub tenant_id: String,
  pub user_id: String,
}

/// `POST /api/v1/signup` request body: create an account *and* mint its first
/// device token in one call, so a client (the PWA above all) can go from
/// "no account" to "connected" without a detour through the web signup form.
/// The password crosses the wire once, is exchanged for the device token, and
/// is never stored by the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignupRequest {
  pub email: String,
  pub password: String,
  /// Shown as the account's display name; defaults to the email when blank.
  #[serde(default)]
  pub display_name: String,
  #[serde(default)]
  pub device_name: String,
  #[serde(default)]
  pub platform: String,
  /// Stable machine id to bind the first device to, as in
  /// [`RegisterDeviceRequest::machine_id`].
  #[serde(default)]
  pub machine_id: Option<String>,
}

/// `POST /api/v1/signup` response body: the same fields device registration
/// returns, since signup mints a device token too. The token is shown once.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignupResponse {
  pub device_id: String,
  pub token: String,
  pub tenant_id: String,
  pub user_id: String,
}

/// `GET /api/v1/me` response body: the authenticated principal.
/// `default_access` is one of `read_write` / `read` / `none`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeResponse {
  pub tenant_id: String,
  pub user_id: String,
  pub device_id: String,
  /// Whether the caller administers the deployment. The only role distinction
  /// the wire carries; everything finer-grained is a deployment's own concern
  /// and reaches the client through `label` instead.
  pub is_admin: bool,
  pub default_access: String,
  pub read_only: bool,
  pub progress_sync_denied: bool,
  /// An optional free-form account label supplied by the deployment, shown
  /// verbatim by clients (a plain server sends `None` and clients show
  /// nothing). Defaulted so a server that omits it still deserializes.
  #[serde(default)]
  pub label: Option<String>,
}

/// One row of `GET /api/v1/devices`: a device belonging to the caller.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceDto {
  pub id: String,
  pub name: String,
  pub platform: String,
  pub default_access: String,
  pub read_only: bool,
  pub progress_sync_denied: bool,
  pub revoked: bool,
  pub created_at: i64,
  pub last_seen_at: Option<i64>,
}

/// `DELETE /api/v1/devices/{id}` response body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevokeDeviceResponse {
  /// The id of the device that was revoked.
  pub revoked: String,
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;

  #[test]
  fn me_response_round_trips() {
    let me = MeResponse {
      tenant_id: "t".into(),
      user_id: "u".into(),
      device_id: "d".into(),
      is_admin: false,
      default_access: "read_write".into(),
      read_only: false,
      progress_sync_denied: false,
      label: Some("Basic".into()),
    };
    let value = serde_json::to_value(&me).unwrap();
    assert_eq!(value["is_admin"], json!(false));
    assert_eq!(value["default_access"], "read_write");
    assert_eq!(value["label"], "Basic");
    let back: MeResponse = serde_json::from_value(value).unwrap();
    assert!(!back.is_admin);
    assert_eq!(back.label.as_deref(), Some("Basic"));
  }

  #[test]
  fn me_response_without_a_label_deserialises() {
    // A deployment that supplies no account label omits the field entirely.
    let plain: MeResponse = serde_json::from_value(json!({
      "tenant_id": "t", "user_id": "u", "device_id": "d", "is_admin": true,
      "default_access": "read", "read_only": true, "progress_sync_denied": false
    }))
    .unwrap();
    assert_eq!(plain.label, None);
    assert!(plain.is_admin);
  }
}
