//! `SyncClient` methods for the encryption marker and the bulk
//! encrypt/decrypt migrations. Split out of `requests` to keep each file within
//! the repository's per-file line budget.

use hygg_shared::crypto;
use hygg_shared::sync::headers::{MACHINE_ID_HEADER, USER_HEADER};
use hygg_shared::sync::proto;

use super::{SyncClient, UploadError};

impl SyncClient {
  /// Read the account's encryption marker (the public flag + salt + verifier).
  pub fn get_encryption(&self) -> Result<proto::EncryptionState, String> {
    let url = format!("{}/api/v1/encryption", self.base_url);
    self
      .agent
      .get(&url)
      .header("Authorization", &self.bearer())
      .header(USER_HEADER, &self.username)
      .header(MACHINE_ID_HEADER, &self.machine_id)
      .call()
      .map_err(|e| e.to_string())?
      .body_mut()
      .read_json()
      .map_err(|e| e.to_string())
  }

  /// Turn encryption on for the account by publishing the salt + verifier.
  pub fn enable_encryption(
    &self,
    req: &proto::EnableEncryptionRequest,
  ) -> Result<proto::EncryptionState, String> {
    let url = format!("{}/api/v1/encryption", self.base_url);
    self
      .agent
      .put(&url)
      .header("Authorization", &self.bearer())
      .header(USER_HEADER, &self.username)
      .header(MACHINE_ID_HEADER, &self.machine_id)
      .send_json(req)
      .map_err(|e| e.to_string())?
      .body_mut()
      .read_json()
      .map_err(|e| e.to_string())
  }

  /// Turn encryption off for the account (the server stops enforcing
  /// envelopes). The caller still has to decrypt and re-upload its documents.
  pub fn disable_encryption(&self) -> Result<proto::EncryptionState, String> {
    let url = format!("{}/api/v1/encryption", self.base_url);
    self
      .agent
      .delete(&url)
      .header("Authorization", &self.bearer())
      .header(USER_HEADER, &self.username)
      .header(MACHINE_ID_HEADER, &self.machine_id)
      .call()
      .map_err(|e| e.to_string())?
      .body_mut()
      .read_json()
      .map_err(|e| e.to_string())
  }

  /// Upload document bytes verbatim (no sealing), used by the decrypt-all
  /// migration to put plaintext back after encryption is disabled.
  fn put_blob_raw(
    &self,
    content_hash: &str,
    bytes: &[u8],
  ) -> Result<(), UploadError> {
    let url = format!("{}/api/v1/books/{content_hash}/blob", self.base_url);
    self
      .agent
      .put(&url)
      .header("Authorization", &self.bearer())
      .header(USER_HEADER, &self.username)
      .header(MACHINE_ID_HEADER, &self.machine_id)
      .header("Content-Type", "application/octet-stream")
      .send(bytes)
      .map_err(|e| UploadError { permanent: false, message: e.to_string() })?;
    Ok(())
  }

  /// Decrypt every sealed document on the server and re-upload it as plaintext.
  /// Run after [`disable_encryption`], while this client still holds the key.
  /// Returns `(decrypted, skipped, failed)`; an already-plaintext blob or a
  /// non-blob-syncing document is skipped.
  pub fn decrypt_and_reupload_all(
    &self,
  ) -> Result<(usize, usize, usize), String> {
    let books = self.fetch_library()?;
    let (mut done, mut skipped, mut failed) = (0usize, 0usize, 0usize);
    for book in books {
      if !book.sync_mode.syncs_blob() {
        skipped += 1;
        continue;
      }
      let Ok(raw) = self.download_book_raw(&book.content_hash) else {
        failed += 1;
        continue;
      };
      if !crypto::is_envelope(&raw) {
        skipped += 1;
        continue;
      }
      let Ok(plain) = self.open_blob(raw) else {
        failed += 1;
        continue;
      };
      let ok = self
        .upsert_meta(
          &book.content_hash,
          &book.title,
          &book.format,
          plain.len() as i64,
        )
        .and_then(|()| self.put_blob_raw(&book.content_hash, &plain));
      if ok.is_ok() {
        done += 1;
      } else {
        failed += 1;
      }
    }
    Ok((done, skipped, failed))
  }
}
