//! End-to-end encryption settings, persisted in `~/.config/hygg/.env`
//! alongside the server keys.
//!
//! The *secret* (the passphrase, or the strong key the wizard generated) is
//! ideally supplied through the `HYGG_ENCRYPTION_KEY` environment variable so
//! it never has to sit in a file — but the wizard can also write it to the
//! config for convenience. The *salt* and *verifier* are non-secret: they come
//! from the account's server marker and are cached here so the key can be
//! derived and verified offline, without a network round-trip on every read.

use std::collections::HashMap;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use hygg_shared::crypto::{self, EncryptionKey};

/// Environment variable holding the account secret — the recommended place to
/// put it, per the setup docs.
pub const ENCRYPTION_KEY_ENV: &str = "HYGG_ENCRYPTION_KEY";

/// The encryption settings resolved from environment + config file.
#[derive(Default, Clone)]
pub struct EncryptionConfig {
  /// Whether this client should encrypt (mirrors the account's server marker,
  /// set by the wizard).
  pub enabled: bool,
  /// The account secret, from `HYGG_ENCRYPTION_KEY` or the config's
  /// `ENCRYPTION_KEY`. Absent means "enabled but not yet set up on this
  /// client" — the state that triggers the wizard.
  pub secret: Option<String>,
  /// base64 of the account KDF salt (non-secret), cached from the marker.
  pub salt_b64: Option<String>,
  /// base64 of the verifier (non-secret), cached from the marker.
  pub verifier_b64: Option<String>,
}

fn file_values() -> HashMap<String, String> {
  super::get_config_env_path()
    .ok()
    .and_then(|path| dotenvy::from_path_iter(path).ok())
    .map(|iter| iter.filter_map(Result::ok).collect())
    .unwrap_or_default()
}

/// Read the encryption settings. `HYGG_ENCRYPTION_KEY` takes precedence over a
/// config-file `ENCRYPTION_KEY` for the secret.
pub fn load_encryption_config() -> EncryptionConfig {
  let fv = file_values();
  let secret = std::env::var(ENCRYPTION_KEY_ENV)
    .ok()
    .filter(|s| !s.trim().is_empty())
    .or_else(|| super::config_string("ENCRYPTION_KEY", &fv))
    .filter(|s| !s.trim().is_empty());
  EncryptionConfig {
    enabled: super::config_bool("ENCRYPTION", &fv).unwrap_or(false),
    secret,
    salt_b64: super::config_string("ENCRYPTION_SALT", &fv)
      .filter(|s| !s.is_empty()),
    verifier_b64: super::config_string("ENCRYPTION_VERIFIER", &fv)
      .filter(|s| !s.is_empty()),
  }
}

/// Persist encryption settings, preserving every other key in the file. The
/// secret is written only when `write_secret` is set (the wizard's "save to
/// config too" path); otherwise the on-disk `ENCRYPTION_KEY` is left as-is so
/// an env-only setup is never clobbered with a blank.
pub fn save_encryption_config(
  cfg: &EncryptionConfig,
  write_secret: bool,
) -> Result<(), Box<dyn std::error::Error>> {
  let mut managed: Vec<(&str, String)> = vec![
    ("ENCRYPTION", cfg.enabled.to_string()),
    ("ENCRYPTION_SALT", cfg.salt_b64.clone().unwrap_or_default()),
    ("ENCRYPTION_VERIFIER", cfg.verifier_b64.clone().unwrap_or_default()),
  ];
  if write_secret {
    managed.push(("ENCRYPTION_KEY", cfg.secret.clone().unwrap_or_default()));
  }
  super::env_io::write_env_preserving(&managed)
}

impl EncryptionConfig {
  /// Derive and verify the content key, or `None` when this client is not fully
  /// set up (disabled, no secret, no cached salt, or a secret that fails the
  /// verifier — i.e. the wrong passphrase). A `None` here is exactly the
  /// condition that should route the user into the setup wizard.
  pub fn resolve_key(&self) -> Option<EncryptionKey> {
    if !self.enabled {
      return None;
    }
    let secret = self.secret.as_ref()?;
    let salt = STANDARD.decode(self.salt_b64.as_ref()?).ok()?;
    let key = crypto::derive_key(secret.as_bytes(), &salt).ok()?;
    if let Some(v) = &self.verifier_b64 {
      let verifier = STANDARD.decode(v).ok()?;
      if !crypto::check_verifier(&key, &verifier) {
        return None;
      }
    }
    Some(key)
  }

  /// True when the account wants encryption but this client can't yet produce
  /// the key — the trigger for the first-connect wizard.
  pub fn needs_setup(&self) -> bool {
    self.enabled && self.resolve_key().is_none()
  }
}
