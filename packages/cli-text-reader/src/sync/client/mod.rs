//! Blocking HTTP client for the sync server, used only from the background
//! engine thread (never the reader's main thread). Reuses `ureq` (already a
//! dependency) with a request timeout so a stuck connection can't hang sync.
//! Every request and response is a shared `proto` DTO, so the wire contract is
//! checked against the server at compile time.

use std::time::Duration;

use hygg_shared::sync::proto;

use super::annotations::{ServerBookmark, ServerHighlight, ServerNote};
use super::inbound::ServerProgress;
use crate::config::ServerConfig;

mod requests;

/// Everything returned by a single `GET /api/v1/sync/pull` (all entity kinds
/// changed since the cursor), converted into the editor-facing types.
#[derive(Default, Debug)]
pub struct PullResult {
  pub server_time: i64,
  pub progress: Vec<ServerProgress>,
  pub bookmarks: Vec<ServerBookmark>,
  pub highlights: Vec<ServerHighlight>,
  pub notes: Vec<ServerNote>,
}

/// A device minted by `register_device`, ready to be stored in config.
#[derive(Clone, Debug)]
pub struct DeviceRegistration {
  pub device_id: String,
  pub token: String,
}

/// Why a book upload failed, so the engine can decide whether to retry. A
/// `permanent` failure (a 4xx other than timeout/rate-limit) will never succeed
/// on retry — the document is dropped from the queue and the user is told —
/// whereas a transient failure (network, 5xx, timeout, rate-limit) keeps the
/// book queued for the next cycle.
#[derive(Clone, Debug)]
pub struct UploadError {
  pub permanent: bool,
  pub message: String,
}

impl std::fmt::Display for UploadError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(&self.message)
  }
}

impl std::error::Error for UploadError {}

/// Build the shared blocking HTTP agent (one request timeout so a stuck socket
/// can never hang the caller).
fn build_agent() -> ureq::Agent {
  ureq::Agent::config_builder()
    .timeout_global(Some(Duration::from_secs(30)))
    .build()
    .new_agent()
}

/// Exchange a user's email + password for a new per-device API token. Used by
/// the login flow (and headless automation) before any sync can happen. Binds
/// the new device to this machine so its token can't be reused elsewhere.
pub fn register_device(
  server_url: &str,
  email: &str,
  password: &str,
  device_name: &str,
) -> Result<DeviceRegistration, String> {
  let base = server_url.trim_end_matches('/');
  let url = format!("{base}/api/v1/devices/register");
  let request = proto::RegisterDeviceRequest {
    email: email.to_string(),
    password: password.to_string(),
    device_name: device_name.to_string(),
    platform: String::new(),
    machine_id: Some(super::machine::machine_id()),
  };
  let response: proto::RegisterDeviceResponse = build_agent()
    .post(&url)
    .send_json(&request)
    .map_err(|e| e.to_string())?
    .body_mut()
    .read_json()
    .map_err(|e| e.to_string())?;
  Ok(DeviceRegistration {
    device_id: response.device_id,
    token: response.token,
  })
}

pub struct SyncClient {
  agent: ureq::Agent,
  base_url: String,
  token: String,
  username: String,
  machine_id: String,
}

impl SyncClient {
  /// Build a client from config, or `None` when not fully configured (missing
  /// URL, username or token) — in which case sync simply does not run.
  pub fn from_config(config: &ServerConfig) -> Option<SyncClient> {
    let url = config.server_url.clone()?;
    let username = config.username.clone()?;
    let token = config.api_token.clone()?;
    Some(SyncClient {
      agent: build_agent(),
      base_url: url.trim_end_matches('/').to_string(),
      token,
      username,
      machine_id: super::machine::machine_id(),
    })
  }

  fn bearer(&self) -> String {
    format!("Bearer {}", self.token)
  }
}
