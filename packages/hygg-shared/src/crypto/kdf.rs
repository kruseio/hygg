//! Turning a human passphrase into a content key, and proving a passphrase is
//! the right one.
//!
//! The account secret (a passphrase the user picks, or a strong key the wizard
//! generates) is stretched with **Argon2id** and a per-account **salt** into
//! the 256-bit key that seals every document. The salt is not secret: it is
//! stored on the server's encryption marker so every client derives the same
//! key, and it exists only to make precomputation useless.
//!
//! The **verifier** solves a UX problem without weakening anything: a new
//! client needs to know whether the passphrase the user typed is *correct*
//! before it silently produces garbage. It is a fixed sentinel sealed under the
//! derived key; a client re-derives the key and checks it can open the sentinel
//! ([`check_verifier`]). Storing it on the server reveals nothing — opening it
//! still requires the key, which the server never has.

use argon2::Argon2;
use base64::Engine;
use base64::engine::general_purpose::STANDARD_NO_PAD;

use super::{CryptoError, EncryptionKey, decrypt, encrypt, fill_random};

/// KDF identifier recorded alongside the salt, so the parameters a key was
/// derived under are pinned rather than assumed.
pub const KDF_ARGON2ID: &str = "argon2id";

/// The constant a verifier seals. Opening the verifier and recovering these
/// exact bytes is what confirms a passphrase derived the right key.
pub const VERIFIER_PLAINTEXT: &[u8] = b"hygg-encryption-verifier-v1";

/// Salt length in bytes. 16 bytes is comfortably above Argon2's 8-byte floor
/// and matches common practice.
const SALT_LEN: usize = 16;

/// Derive the 256-bit content key from a secret and the account salt with
/// Argon2id (the crate defaults: 19 MiB, 2 passes, 1 lane). Deterministic —
/// the same secret and salt always yield the same key, which is what makes the
/// key portable across clients.
pub fn derive_key(
  secret: &[u8],
  salt: &[u8],
) -> Result<EncryptionKey, CryptoError> {
  let mut key = [0u8; 32];
  Argon2::default()
    .hash_password_into(secret, salt, &mut key)
    .map_err(|_| CryptoError::Decrypt)?;
  Ok(EncryptionKey::from_bytes(key))
}

/// A fresh random account salt. Generated once, when encryption is first
/// enabled, then stored (non-secret) on the server marker.
pub fn random_salt() -> Result<[u8; SALT_LEN], CryptoError> {
  let mut salt = [0u8; SALT_LEN];
  fill_random(&mut salt)?;
  Ok(salt)
}

/// A strong, copy-pasteable random key phrase for the "generate one for me"
/// path: 32 bytes of entropy rendered as unpadded base64 (~43 chars). The user
/// stores this in a password manager; it is used as the passphrase secret.
pub fn generate_key_phrase() -> Result<String, CryptoError> {
  let mut raw = [0u8; 32];
  fill_random(&mut raw)?;
  Ok(STANDARD_NO_PAD.encode(raw))
}

/// Seal the sentinel under `key`, producing the verifier stored on the marker.
pub fn make_verifier(key: &EncryptionKey) -> Result<Vec<u8>, CryptoError> {
  encrypt(key, VERIFIER_PLAINTEXT)
}

/// Whether `key` opens `verifier` back to the sentinel — i.e. whether the
/// passphrase this key came from is the account's passphrase. Any failure
/// (wrong key, malformed verifier) is a plain `false`.
pub fn check_verifier(key: &EncryptionKey, verifier: &[u8]) -> bool {
  decrypt(key, verifier).map(|pt| pt == VERIFIER_PLAINTEXT).unwrap_or(false)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn same_secret_and_salt_derive_the_same_key() {
    let salt = random_salt().unwrap();
    let a = derive_key(b"correct horse", &salt).unwrap();
    let b = derive_key(b"correct horse", &salt).unwrap();
    // Prove equality via a round-trip: `a` seals, `b` opens.
    let sealed = super::encrypt(&a, b"hello").unwrap();
    assert_eq!(super::decrypt(&b, &sealed).unwrap(), b"hello");
  }

  #[test]
  fn different_salt_derives_a_different_key() {
    let k1 = derive_key(b"pw", &random_salt().unwrap()).unwrap();
    let k2 = derive_key(b"pw", &random_salt().unwrap()).unwrap();
    let sealed = super::encrypt(&k1, b"hi").unwrap();
    assert!(super::decrypt(&k2, &sealed).is_err());
  }

  #[test]
  fn verifier_accepts_the_right_key_and_rejects_others() {
    let salt = random_salt().unwrap();
    let key = derive_key(b"the passphrase", &salt).unwrap();
    let verifier = make_verifier(&key).unwrap();

    let same = derive_key(b"the passphrase", &salt).unwrap();
    assert!(check_verifier(&same, &verifier));

    let wrong = derive_key(b"a different passphrase", &salt).unwrap();
    assert!(!check_verifier(&wrong, &verifier));
  }

  #[test]
  fn check_verifier_rejects_garbage_bytes() {
    let key = derive_key(b"pw", &random_salt().unwrap()).unwrap();
    assert!(!check_verifier(&key, b"not a verifier"));
    assert!(!check_verifier(&key, &[]));
  }

  #[test]
  fn generated_phrases_are_unique_and_nonempty() {
    let a = generate_key_phrase().unwrap();
    let b = generate_key_phrase().unwrap();
    assert!(!a.is_empty());
    assert_ne!(a, b);
  }
}
