//! Per-device API tokens. A token is `"{prefix}.{secret}"`: the `prefix` is a
//! public lookup key (stored and indexed), the `secret` is 256 bits of entropy
//! shown to the client exactly once. Only `sha256(secret)` is stored; lookups
//! verify in constant time. The high entropy is why SHA-256 (not Argon2) is
//! sufficient here.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::RngCore;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

pub struct GeneratedToken {
  /// The full `prefix.secret` shown to the client once.
  pub full: String,
  /// Public lookup key stored in `api_tokens.prefix`.
  pub prefix: String,
  /// `sha256(secret)` hex, stored in `api_tokens.token_hash`.
  pub hash: String,
}

/// Mint a fresh token.
pub fn generate_token() -> GeneratedToken {
  let mut prefix_bytes = [0u8; 6];
  let mut secret_bytes = [0u8; 32];
  rand::thread_rng().fill_bytes(&mut prefix_bytes);
  rand::thread_rng().fill_bytes(&mut secret_bytes);
  let prefix = URL_SAFE_NO_PAD.encode(prefix_bytes);
  let secret = URL_SAFE_NO_PAD.encode(secret_bytes);
  let hash = hash_secret(&secret);
  GeneratedToken { full: format!("{prefix}.{secret}"), prefix, hash }
}

/// Split a presented `prefix.secret` token.
pub fn split_token(full: &str) -> Option<(&str, &str)> {
  full.split_once('.')
}

/// Lowercase hex SHA-256 of a token secret.
pub fn hash_secret(secret: &str) -> String {
  let digest = Sha256::digest(secret.as_bytes());
  digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// Constant-time check that `secret` matches a stored `token_hash`.
pub fn verify_secret(secret: &str, stored_hash: &str) -> bool {
  let computed = hash_secret(secret);
  computed.as_bytes().ct_eq(stored_hash.as_bytes()).into()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn generated_token_verifies_against_its_hash() {
    let token = generate_token();
    let (prefix, secret) = split_token(&token.full).unwrap();
    assert_eq!(prefix, token.prefix);
    assert!(verify_secret(secret, &token.hash));
    assert!(!verify_secret("tampered", &token.hash));
  }

  #[test]
  fn tokens_are_distinct() {
    assert_ne!(generate_token().full, generate_token().full);
  }

  #[test]
  fn split_requires_a_dot() {
    assert!(split_token("nodot").is_none());
    assert_eq!(split_token("a.b"), Some(("a", "b")));
  }
}
