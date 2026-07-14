//! A stable per-machine identity, sent with every authenticated sync request.
//! The server binds a device token to the first machine id it sees and rejects
//! the token from any other machine, so one token can't be shared across
//! machines.
//!
//! The id is derived from an OS-provided machine identity where one exists
//! (Linux `/etc/machine-id`, macOS `IOPlatformUUID`) — hashed with a domain
//! prefix so the raw hardware id never leaves the device — and otherwise falls
//! back to a random id persisted under the hygg config dir. Either way it is
//! stable across runs on the same machine.

use hygg_shared::sync::content_sha256;
use uuid::Uuid;

use crate::utils::get_hygg_config_file;

/// The machine id for this install (see module docs). Never fails: on any error
/// it returns a fresh random id (which merely re-binds on next use).
pub fn machine_id() -> String {
  if let Some(raw) = os_machine_id() {
    return content_sha256(format!("hygg-machine-v1:{raw}").as_bytes());
  }
  persisted_fallback_id()
}

/// An OS-provided stable machine identifier, if available.
fn os_machine_id() -> Option<String> {
  for path in ["/etc/machine-id", "/var/lib/dbus/machine-id"] {
    if let Ok(contents) = std::fs::read_to_string(path) {
      let trimmed = contents.trim();
      if !trimmed.is_empty() {
        return Some(trimmed.to_string());
      }
    }
  }
  #[cfg(target_os = "macos")]
  if let Some(uuid) = macos_platform_uuid() {
    return Some(uuid);
  }
  None
}

/// macOS hardware UUID from `ioreg` (the `IOPlatformUUID` property).
#[cfg(target_os = "macos")]
fn macos_platform_uuid() -> Option<String> {
  let output = std::process::Command::new("ioreg")
    .args(["-rd1", "-c", "IOPlatformExpertDevice"])
    .output()
    .ok()?;
  let text = String::from_utf8_lossy(&output.stdout);
  let line = text.lines().find(|line| line.contains("IOPlatformUUID"))?;
  let value = line.split('=').nth(1)?.trim().trim_matches('"');
  (!value.is_empty()).then(|| value.to_string())
}

/// A random id created once and persisted under the config dir, used when the
/// OS exposes no stable machine identity.
fn persisted_fallback_id() -> String {
  let Ok(path) = get_hygg_config_file("machine-id") else {
    return Uuid::new_v4().to_string();
  };
  if let Ok(existing) = std::fs::read_to_string(&path) {
    let trimmed = existing.trim();
    if !trimmed.is_empty() {
      return trimmed.to_string();
    }
  }
  let id = Uuid::new_v4().to_string();
  let _ = std::fs::write(&path, &id);
  id
}
