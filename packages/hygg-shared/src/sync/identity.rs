//! Stable, content-derived identity for a document ("book").
//!
//! The reader's local `document_hash: u64` comes from a non-cryptographic
//! `DefaultHasher` and is computed inconsistently across code paths (the PDF
//! path hashes the canonical file path, the content path hashes the text), so
//! it is unsuitable as a cross-device key. `book_id_from_text` derives a stable
//! identity — the SHA-256 of the extracted text — that is identical for the
//! same document on any machine and any Rust version. The sync server keys
//! books under this value; the local `u64` stays purely local (file naming).

use sha2::{Digest, Sha256};

/// A book's stable sync identity: the lowercase hex SHA-256 of its extracted
/// text.
pub fn book_id_from_text(text: &str) -> String {
  content_sha256(text.as_bytes())
}

/// A book's stable sync identity derived from a source file's bytes — the same
/// document on any device yields the same id, regardless of how its text is
/// later extracted. Returns `None` if the file can't be read.
pub fn book_id_for_file(path: &std::path::Path) -> Option<String> {
  std::fs::read(path).ok().map(|bytes| content_sha256(&bytes))
}

/// Lowercase hex SHA-256 of arbitrary bytes.
pub fn content_sha256(bytes: &[u8]) -> String {
  let digest = Sha256::digest(bytes);
  digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn content_sha256_matches_known_vectors() {
    assert_eq!(
      content_sha256(b""),
      "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
      content_sha256(b"abc"),
      "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
  }

  #[test]
  fn book_id_is_64_hex_chars_and_deterministic() {
    let id = book_id_from_text("hello world");
    assert_eq!(id.len(), 64);
    assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(id, book_id_from_text("hello world"));
  }

  #[test]
  fn different_text_yields_different_id() {
    assert_ne!(book_id_from_text("alpha"), book_id_from_text("beta"));
  }
}
