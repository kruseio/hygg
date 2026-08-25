//! Encrypting a text field that travels inside JSON.
//!
//! A document blob is raw bytes, so its envelope ships as-is. A note body is a
//! JSON string, so its envelope is base64-encoded and given a short ASCII
//! prefix. The prefix serves the same purpose the binary magic does for blobs:
//! it lets any party — including the server, which cannot decrypt — recognize
//! an encrypted field and enforce that it is one, and it lets a client tell an
//! already-encrypted body from a plaintext one during migration.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;

use super::{CryptoError, EncryptionKey, decrypt, encrypt, is_envelope};

/// Marker that opens every encrypted string field. Chosen to be something a
/// human note body would never begin with.
pub const STRING_PREFIX: &str = "hygge1:";

/// Whether `s` is an encrypted string field (has the marker and a plausible
/// envelope after it). Used by the server to enforce that note bodies are
/// encrypted, and by clients to avoid double-encrypting during conversion.
pub fn is_encrypted_string(s: &str) -> bool {
  match s.strip_prefix(STRING_PREFIX) {
    Some(b64) => STANDARD.decode(b64).map(|v| is_envelope(&v)).unwrap_or(false),
    None => false,
  }
}

/// Seal a text field: `hygge1:` + base64(envelope). An empty string stays empty
/// — there is nothing to hide, and keeping it empty preserves the "empty body
/// means a tombstone/no-op" handling on both ends.
pub fn encrypt_string(
  key: &EncryptionKey,
  plaintext: &str,
) -> Result<String, CryptoError> {
  if plaintext.is_empty() {
    return Ok(String::new());
  }
  let sealed = encrypt(key, plaintext.as_bytes())?;
  Ok(format!("{STRING_PREFIX}{}", STANDARD.encode(sealed)))
}

/// Open a text field produced by [`encrypt_string`]. A string without the
/// marker is returned unchanged — during and after migration a store can hold
/// both, and a plaintext body is already readable. Invalid UTF-8 after
/// decryption is reported as [`CryptoError::Decrypt`].
pub fn decrypt_string(
  key: &EncryptionKey,
  value: &str,
) -> Result<String, CryptoError> {
  let Some(b64) = value.strip_prefix(STRING_PREFIX) else {
    return Ok(value.to_string());
  };
  let sealed = STANDARD.decode(b64).map_err(|_| CryptoError::Decrypt)?;
  let plaintext = decrypt(key, &sealed)?;
  String::from_utf8(plaintext).map_err(|_| CryptoError::Decrypt)
}

#[cfg(test)]
mod tests {
  use super::super::derive_key;
  use super::*;

  fn key() -> EncryptionKey {
    derive_key(b"pw", b"0123456789abcdef").unwrap()
  }

  #[test]
  fn round_trips_a_note_body() {
    let k = key();
    let sealed = encrypt_string(&k, "a private note").unwrap();
    assert!(sealed.starts_with(STRING_PREFIX));
    assert!(is_encrypted_string(&sealed));
    assert_eq!(decrypt_string(&k, &sealed).unwrap(), "a private note");
  }

  #[test]
  fn empty_stays_empty() {
    let k = key();
    assert_eq!(encrypt_string(&k, "").unwrap(), "");
    assert!(!is_encrypted_string(""));
    assert_eq!(decrypt_string(&k, "").unwrap(), "");
  }

  #[test]
  fn plaintext_passes_through_decrypt_unchanged() {
    let k = key();
    // A body written before encryption was enabled has no marker.
    assert_eq!(decrypt_string(&k, "legacy body").unwrap(), "legacy body");
    assert!(!is_encrypted_string("legacy body"));
  }

  #[test]
  fn unicode_survives() {
    let k = key();
    let msg = "café — naïve — 日本語 — 🔐";
    let sealed = encrypt_string(&k, msg).unwrap();
    assert_eq!(decrypt_string(&k, &sealed).unwrap(), msg);
  }

  #[test]
  fn wrong_key_fails() {
    let sealed = encrypt_string(&key(), "secret").unwrap();
    let other = derive_key(b"other", b"0123456789abcdef").unwrap();
    assert_eq!(decrypt_string(&other, &sealed), Err(CryptoError::Decrypt));
  }

  #[test]
  fn marker_with_bad_base64_is_not_encrypted() {
    assert!(!is_encrypted_string("hygge1:not+valid+envelope"));
  }
}
