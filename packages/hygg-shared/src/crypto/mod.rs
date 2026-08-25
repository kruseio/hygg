//! End-to-end document encryption for hygg.
//!
//! One account passphrase, entered on every client, is stretched by Argon2id
//! (see [`kdf`]) into a 256-bit [`EncryptionKey`]. Document bytes and note
//! bodies are sealed under that key with XChaCha20-Poly1305 into a versioned
//! *envelope*; the server stores the envelope verbatim and never holds the key,
//! so it can neither read the content nor forge one. Because the key derivation
//! and the envelope live here in the MIT `hygg-shared` crate, every client and
//! the server agree on the exact same bytes.
//!
//! The envelope is self-describing and magic-prefixed so anyone — including the
//! server, which cannot decrypt it — can cheaply tell an encrypted blob from a
//! plaintext one ([`is_envelope`]). That check is what lets the server
//! *enforce* encryption: once an account turns it on, an upload that is not an
//! envelope is rejected, so a client that has not been set up cannot push
//! readable bytes.
//!
//! ```text
//! byte  0 ..  6   magic  "HYGGE1"
//! byte  6         alg    (0x01 = XChaCha20-Poly1305)
//! byte  7 .. 31   nonce  (24 random bytes, unique per message)
//! byte 31 ..      ciphertext ‖ 16-byte Poly1305 tag
//! ```
//!
//! Losing the passphrase means losing the data: there is no recovery path, by
//! design. The clients' setup wizards make that consequence loud and push the
//! user to store the key in a password manager.

use chacha20poly1305::aead::Aead;
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce};
use zeroize::Zeroizing;

mod kdf;
mod text;

pub use kdf::{
  KDF_ARGON2ID, VERIFIER_PLAINTEXT, check_verifier, derive_key,
  generate_key_phrase, make_verifier, random_salt,
};
pub use text::{decrypt_string, encrypt_string, is_encrypted_string};

/// Envelope magic: the first six bytes of every sealed blob. Versioned (the
/// trailing `1`) so a future format change is distinguishable rather than
/// silently misparsed.
pub const MAGIC: &[u8; 6] = b"HYGGE1";

/// Algorithm id in byte 6. Only XChaCha20-Poly1305 exists today; the field
/// leaves room to migrate without a second magic.
pub const ALG_XCHACHA20POLY1305: u8 = 0x01;

/// XChaCha20-Poly1305 nonce length.
const NONCE_LEN: usize = 24;

/// Bytes before the ciphertext: magic (6) + alg (1) + nonce (24).
const HEADER_LEN: usize = MAGIC.len() + 1 + NONCE_LEN;

/// A derived 256-bit content key, zeroized on drop so it does not linger in
/// freed memory. Build one with [`derive_key`]; it is never serialized.
#[derive(Clone)]
pub struct EncryptionKey(Zeroizing<[u8; 32]>);

impl EncryptionKey {
  /// Wrap raw key bytes (used by [`derive_key`] and tests). Prefer deriving
  /// from a passphrase over constructing this directly.
  pub fn from_bytes(bytes: [u8; 32]) -> Self {
    EncryptionKey(Zeroizing::new(bytes))
  }

  fn cipher(&self) -> XChaCha20Poly1305 {
    // `new_from_slice` only fails on a wrong length; the array is always 32.
    XChaCha20Poly1305::new_from_slice(&self.0[..])
      .expect("32-byte key is always valid")
  }
}

impl std::fmt::Debug for EncryptionKey {
  /// Never print key material, even by accident in a log line.
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str("EncryptionKey(<redacted>)")
  }
}

/// Why an envelope could not be opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CryptoError {
  /// Randomness was unavailable (only possible on a broken platform RNG).
  Rng,
  /// The bytes are not a hygg envelope (bad/absent magic, alg, or too short).
  NotEnvelope,
  /// The envelope is well-formed but the key is wrong or the bytes were
  /// tampered with — the AEAD tag did not verify.
  Decrypt,
}

impl std::fmt::Display for CryptoError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let msg = match self {
      CryptoError::Rng => "secure randomness is unavailable",
      CryptoError::NotEnvelope => "not a hygg encrypted envelope",
      CryptoError::Decrypt => "could not decrypt (wrong key or corrupted data)",
    };
    f.write_str(msg)
  }
}

impl std::error::Error for CryptoError {}

/// Fill `buf` with cryptographically secure random bytes (OS RNG natively,
/// `crypto.getRandomValues` in the browser).
pub(crate) fn fill_random(buf: &mut [u8]) -> Result<(), CryptoError> {
  getrandom::getrandom(buf).map_err(|_| CryptoError::Rng)
}

/// Whether `bytes` is a hygg encrypted envelope. A cheap prefix check the
/// server uses to enforce encryption without ever holding the key.
pub fn is_envelope(bytes: &[u8]) -> bool {
  bytes.len() >= HEADER_LEN
    && &bytes[..MAGIC.len()] == MAGIC
    && bytes[MAGIC.len()] == ALG_XCHACHA20POLY1305
}

/// Seal `plaintext` under `key`, returning a self-describing envelope. Each
/// call draws a fresh random nonce, so encrypting the same bytes twice yields
/// different envelopes.
pub fn encrypt(
  key: &EncryptionKey,
  plaintext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
  let mut nonce = [0u8; NONCE_LEN];
  fill_random(&mut nonce)?;
  let ciphertext = key
    .cipher()
    .encrypt(XNonce::from_slice(&nonce), plaintext)
    .map_err(|_| CryptoError::Decrypt)?;

  let mut out = Vec::with_capacity(HEADER_LEN + ciphertext.len());
  out.extend_from_slice(MAGIC);
  out.push(ALG_XCHACHA20POLY1305);
  out.extend_from_slice(&nonce);
  out.extend_from_slice(&ciphertext);
  Ok(out)
}

/// Open an envelope produced by [`encrypt`]. Fails with
/// [`CryptoError::Decrypt`] on a wrong key or any tampering (the Poly1305 tag
/// is checked), and with [`CryptoError::NotEnvelope`] on bytes that are not an
/// envelope at all.
pub fn decrypt(
  key: &EncryptionKey,
  envelope: &[u8],
) -> Result<Vec<u8>, CryptoError> {
  if !is_envelope(envelope) {
    return Err(CryptoError::NotEnvelope);
  }
  let nonce = &envelope[MAGIC.len() + 1..HEADER_LEN];
  let ciphertext = &envelope[HEADER_LEN..];
  key
    .cipher()
    .decrypt(XNonce::from_slice(nonce), ciphertext)
    .map_err(|_| CryptoError::Decrypt)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn test_key() -> EncryptionKey {
    EncryptionKey::from_bytes([7u8; 32])
  }

  #[test]
  fn round_trips_plaintext() {
    let key = test_key();
    let msg = b"the quick brown fox";
    let sealed = encrypt(&key, msg).unwrap();
    assert!(is_envelope(&sealed));
    assert_eq!(decrypt(&key, &sealed).unwrap(), msg);
  }

  #[test]
  fn round_trips_empty_input() {
    let key = test_key();
    let sealed = encrypt(&key, b"").unwrap();
    assert!(is_envelope(&sealed));
    assert_eq!(decrypt(&key, &sealed).unwrap(), b"");
  }

  #[test]
  fn nonce_is_random_so_ciphertext_differs() {
    let key = test_key();
    let a = encrypt(&key, b"same").unwrap();
    let b = encrypt(&key, b"same").unwrap();
    assert_ne!(a, b, "reused nonce would be a fatal AEAD misuse");
    assert_eq!(decrypt(&key, &a).unwrap(), decrypt(&key, &b).unwrap());
  }

  #[test]
  fn wrong_key_fails_to_decrypt() {
    let sealed = encrypt(&test_key(), b"secret").unwrap();
    let other = EncryptionKey::from_bytes([9u8; 32]);
    assert_eq!(decrypt(&other, &sealed), Err(CryptoError::Decrypt));
  }

  #[test]
  fn tampering_is_detected() {
    let key = test_key();
    let mut sealed = encrypt(&key, b"secret").unwrap();
    let last = sealed.len() - 1;
    sealed[last] ^= 0x01;
    assert_eq!(decrypt(&key, &sealed), Err(CryptoError::Decrypt));
  }

  #[test]
  fn plaintext_is_not_an_envelope() {
    assert!(!is_envelope(b""));
    assert!(!is_envelope(b"HYGGE1")); // magic only, too short
    assert!(!is_envelope(b"a plain document body that is long enough"));
    assert_eq!(
      decrypt(&test_key(), b"not an envelope at all really"),
      Err(CryptoError::NotEnvelope)
    );
  }

  #[test]
  fn unknown_alg_is_not_an_envelope() {
    let mut sealed = encrypt(&test_key(), b"x").unwrap();
    sealed[MAGIC.len()] = 0xFF; // corrupt the alg byte
    assert!(!is_envelope(&sealed));
  }

  #[test]
  fn debug_never_leaks_key_bytes() {
    let dbg = format!("{:?}", test_key());
    assert_eq!(dbg, "EncryptionKey(<redacted>)");
  }
}
