//! Account encryption marker over `/api/v1/encryption`, and the "convert my
//! existing documents" migration. The key itself never leaves the browser —
//! these calls carry only the public flag/salt/verifier and, for conversion,
//! locally sealed blobs.

use gloo_net::http::Request;
use hygg_shared::sync::proto::{EnableEncryptionRequest, EncryptionState};

use super::{Creds, Res, api, authed, error_body};
use crate::storage;

/// Read the account's encryption marker (flag + salt + verifier).
pub async fn get_encryption(creds: &Creds) -> Res<EncryptionState> {
  let resp = authed(Request::get(&api(&creds.server, "/encryption")), creds)
    .send()
    .await
    .map_err(|e| e.to_string())?;
  if !resp.ok() {
    return Err(error_body(resp).await);
  }
  resp.json().await.map_err(|e| e.to_string())
}

/// Turn encryption on for the account by publishing the salt + verifier.
pub async fn enable_encryption(
  creds: &Creds,
  req: &EnableEncryptionRequest,
) -> Res<EncryptionState> {
  let resp = authed(Request::put(&api(&creds.server, "/encryption")), creds)
    .json(req)
    .map_err(|e| e.to_string())?
    .send()
    .await
    .map_err(|e| e.to_string())?;
  if !resp.ok() {
    return Err(error_body(resp).await);
  }
  resp.json().await.map_err(|e| e.to_string())
}

/// Turn encryption off for the account (the server stops enforcing envelopes).
/// The caller re-uploads its documents as plaintext afterwards.
pub async fn disable_encryption(creds: &Creds) -> Res<EncryptionState> {
  let resp = authed(Request::delete(&api(&creds.server, "/encryption")), creds)
    .send()
    .await
    .map_err(|e| e.to_string())?;
  if !resp.ok() {
    return Err(error_body(resp).await);
  }
  resp.json().await.map_err(|e| e.to_string())
}

/// Re-upload every locally-stored, blob-syncing document sealed under the
/// account key (which `creds.key` must hold). Returns `(sealed, skipped)`. A
/// document whose bytes don't sync, or that has no local blob, is skipped.
pub async fn convert_library(creds: &Creds) -> Res<(usize, usize)> {
  let summaries = storage::list_library().await.map_err(|e| e.to_string())?;
  let (mut sealed, mut skipped) = (0usize, 0usize);
  for summary in summaries {
    if !summary.effective_sync_mode().syncs_blob() {
      skipped += 1;
      continue;
    }
    let Some(bytes) = storage::get_blob(&summary.id).await else {
      skipped += 1;
      continue;
    };
    // `upload_book` seals the bytes because `creds.key` is set.
    match super::upload_book(
      creds,
      &summary.id,
      &summary.title,
      &summary.format,
      &bytes,
    )
    .await
    {
      Ok(()) => sealed += 1,
      Err(_) => skipped += 1,
    }
  }
  Ok((sealed, skipped))
}
