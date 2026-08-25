//! Account encryption endpoints: read the marker, and turn encryption on.
//!
//! These carry only the *public* half of end-to-end encryption — the flag, the
//! KDF, the salt, and a verifier. The key never reaches the server, so nothing
//! here can decrypt a document. `GET` is what a freshly connected client calls
//! to discover it must run the setup wizard; `PUT` is how the first client that
//! enables encryption publishes the salt/verifier the others will need.

use axum::Json;
use axum::extract::State;
use hygg_shared::sync::proto::{EnableEncryptionRequest, EncryptionState};

use crate::error::{AppError, AppResult};
use crate::middleware::entitlement::SyncPrincipal;
use crate::repo;
use crate::repo::encryption::EnableOutcome;
use crate::state::AppState;

/// `GET /api/v1/encryption` — the caller's account encryption marker. A
/// `disabled` state (the default for a fresh account) carries empty material.
pub async fn get_encryption(
  SyncPrincipal(principal): SyncPrincipal,
  State(state): State<AppState>,
) -> AppResult<Json<EncryptionState>> {
  let marker = repo::encryption::get(
    &state.db.conn,
    &principal.tenant_id,
    &principal.user_id,
  )
  .await?;
  Ok(Json(marker.map(to_state).unwrap_or_default()))
}

/// `PUT /api/v1/encryption` — turn encryption on for the account by publishing
/// the salt + verifier. Idempotent for a resent identical salt; a *different*
/// salt on an already-enabled account is a conflict (it would strand every
/// document sealed under the original key).
pub async fn put_encryption(
  SyncPrincipal(principal): SyncPrincipal,
  State(state): State<AppState>,
  Json(req): Json<EnableEncryptionRequest>,
) -> AppResult<Json<EncryptionState>> {
  if req.salt.trim().is_empty() || req.verifier.trim().is_empty() {
    return Err(AppError::BadRequest(
      "salt and verifier are required to enable encryption".to_string(),
    ));
  }
  let outcome = repo::encryption::enable(
    &state.db.conn,
    &principal.tenant_id,
    &principal.user_id,
    &req.kdf,
    req.alg as i64,
    &req.salt,
    &req.verifier,
  )
  .await?;
  match outcome {
    EnableOutcome::Set(model) => Ok(Json(to_state(model))),
    EnableOutcome::SaltConflict => Err(AppError::Conflict(
      "encryption is already enabled for this account under a different key; \
       use the existing key rather than re-enabling"
        .to_string(),
    )),
  }
}

/// Enforce that an uploaded blob is a sealed envelope when the account requires
/// encryption. Returns whether the body is encrypted (so the caller can skip
/// server-side extraction on ciphertext).
pub async fn enforce_blob_encrypted(
  db: &sea_orm::DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
  body: &[u8],
) -> AppResult<bool> {
  let encrypted = hygg_shared::crypto::is_envelope(body);
  if !encrypted && repo::encryption::is_enabled(db, tenant_id, user_id).await? {
    return Err(AppError::Conflict(
      "account encryption is enabled; this document must be uploaded \
       encrypted. Set up encryption on this client first."
        .to_string(),
    ));
  }
  Ok(encrypted)
}

/// `DELETE /api/v1/encryption` — turn encryption off for the account. The
/// server stops enforcing envelopes; a client is responsible for decrypting and
/// re-uploading its documents as plaintext.
pub async fn delete_encryption(
  SyncPrincipal(principal): SyncPrincipal,
  State(state): State<AppState>,
) -> AppResult<Json<EncryptionState>> {
  repo::encryption::disable(
    &state.db.conn,
    &principal.tenant_id,
    &principal.user_id,
  )
  .await?;
  let marker = repo::encryption::get(
    &state.db.conn,
    &principal.tenant_id,
    &principal.user_id,
  )
  .await?;
  Ok(Json(marker.map(to_state).unwrap_or_default()))
}

fn to_state(m: crate::entity::encryption_markers::Model) -> EncryptionState {
  EncryptionState {
    enabled: m.enabled != 0,
    kdf: m.kdf,
    alg: m.alg as u8,
    salt: m.salt,
    verifier: m.verifier,
  }
}
