//! Account encryption-marker shapes.
//!
//! The marker is the *public* half of end-to-end encryption: whether an account
//! has it turned on, and the non-secret material every client needs to derive
//! the same key (the KDF, the salt) plus a verifier to confirm a typed
//! passphrase is correct. The key itself never appears here — the server stores
//! and serves only what cannot decrypt anything.

use serde::{Deserialize, Serialize};

/// `GET /api/v1/encryption` response, and the body echoed by `PUT`.
///
/// `salt` and `verifier` are base64 (standard alphabet) and are empty strings
/// when `enabled` is false. `serde(default)` throughout keeps a client that
/// predates a field decoding cleanly.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncryptionState {
  /// Whether the account requires encrypted uploads. Once true, the server
  /// rejects any blob or note body that is not a sealed envelope.
  pub enabled: bool,
  /// Key-derivation function id (e.g. `argon2id`). Empty when disabled.
  #[serde(default)]
  pub kdf: String,
  /// Envelope algorithm id the account was set up with (see
  /// `hygg_shared::crypto::ALG_XCHACHA20POLY1305`). 0 when disabled.
  #[serde(default)]
  pub alg: u8,
  /// base64 of the per-account KDF salt (non-secret). Empty when disabled.
  #[serde(default)]
  pub salt: String,
  /// base64 of the verifier: a fixed sentinel sealed under the derived key, so
  /// a new client can confirm a typed passphrase before trusting it. Empty
  /// when disabled.
  #[serde(default)]
  pub verifier: String,
}

/// `PUT /api/v1/encryption` request body: turn encryption on for the account by
/// publishing the salt + verifier the first client generated. Idempotent — a
/// resend with the same salt/verifier is accepted; a *different* salt on an
/// already-enabled account is refused, since it would strand every document
/// sealed under the original key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnableEncryptionRequest {
  pub kdf: String,
  pub alg: u8,
  /// base64 of the per-account KDF salt.
  pub salt: String,
  /// base64 of the verifier sentinel.
  pub verifier: String,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn disabled_state_round_trips_with_empty_material() {
    let state = EncryptionState::default();
    let json = serde_json::to_string(&state).unwrap();
    let back: EncryptionState = serde_json::from_str(&json).unwrap();
    assert_eq!(state, back);
    assert!(!back.enabled);
    assert!(back.salt.is_empty());
  }

  #[test]
  fn older_client_missing_fields_defaults_cleanly() {
    // A payload that predates every optional field still decodes.
    let back: EncryptionState =
      serde_json::from_str(r#"{"enabled":true}"#).unwrap();
    assert!(back.enabled);
    assert_eq!(back.alg, 0);
    assert!(back.verifier.is_empty());
  }
}
