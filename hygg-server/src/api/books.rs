//! Document endpoints: list the caller's documents, register/update document
//! metadata, and upload/download the document bytes (keyed by the client's
//! content hash = cross-device `book_id`). Device access is evaluated per
//! document.

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use hygg_shared::sync::proto;
use sha2::{Digest, Sha256};

use crate::error::{AppError, AppResult};
use crate::middleware::entitlement::SyncPrincipal;
use crate::repo;
use crate::state::AppState;

/// `GET /api/v1/books`
pub async fn list_books(
  SyncPrincipal(principal): SyncPrincipal,
  State(state): State<AppState>,
) -> AppResult<Json<Vec<proto::BookDto>>> {
  let rows = repo::books::list_for_user(
    &state.db.conn,
    &principal.tenant_id,
    &principal.user_id,
  )
  .await?;
  let is_admin = principal.role.is_admin();
  let mut visible = Vec::with_capacity(rows.len());
  for row in rows {
    if !principal.can_read_book(&row.content_hash) {
      continue;
    }
    let access = repo::access::library(
      &state.db.conn,
      state.entitlements.as_ref(),
      &principal.tenant_id,
      &principal.user_id,
      is_admin,
      principal.personal_sync,
      Some(&principal.device_id),
      &row.owner_user_id,
      row.organization_id.as_deref(),
      row.directory_id.as_deref(),
      &row.content_hash,
    )
    .await?;
    if access.can_read() {
      visible.push(row);
    }
  }
  Ok(Json(visible.into_iter().map(Into::into).collect()))
}

/// `POST /api/v1/books`
pub async fn upsert_book(
  SyncPrincipal(principal): SyncPrincipal,
  State(state): State<AppState>,
  Json(req): Json<proto::UpsertBookRequest>,
) -> AppResult<Json<proto::UpsertBookResponse>> {
  if !principal.can_write_book(&req.content_hash) {
    return Err(AppError::Forbidden);
  }
  // A brand-new content hash is a personal upload by the caller (allowed). An
  // existing document requires write access under the permission model — for
  // org documents that means read_write, for personal ones it means ownership.
  let existing = repo::books::access_meta(
    &state.db.conn,
    &principal.tenant_id,
    &req.content_hash,
  )
  .await?;
  if let Some(meta) = &existing {
    let access = repo::access::library(
      &state.db.conn,
      state.entitlements.as_ref(),
      &principal.tenant_id,
      &principal.user_id,
      principal.role.is_admin(),
      principal.personal_sync,
      Some(&principal.device_id),
      &meta.owner_user_id,
      meta.organization_id.as_deref(),
      meta.directory_id.as_deref(),
      &req.content_hash,
    )
    .await?;
    if !access.can_write() {
      return Err(AppError::Forbidden);
    }
  }
  // Storage budget, unlimited unless an override says otherwise. The
  // document counts against the org's shared pool for org documents, else the
  // uploader's personal pool — the hook resolves which and enforces the cap.
  state
    .entitlements
    .authorize_upload(crate::ext::UploadCtx {
      tenant_id: &principal.tenant_id,
      user_id: &principal.user_id,
      organization_id: existing
        .as_ref()
        .and_then(|meta| meta.organization_id.as_deref()),
      content_hash: &req.content_hash,
      requested_size: req.size_bytes,
    })
    .await?;
  let input = repo::books::BookInput {
    content_hash: &req.content_hash,
    title: &req.title,
    author: &req.author,
    format: &req.format,
    size_bytes: req.size_bytes,
  };
  repo::books::upsert(
    &state.db.conn,
    &principal.tenant_id,
    &principal.user_id,
    &input,
  )
  .await?;
  // An explicit `sync_mode` on the upsert also moves the account-wide ceiling
  // (write access was already established above). Omitting it — the common
  // case — leaves any existing policy untouched.
  if let Some(mode) = req.sync_mode {
    repo::books::set_sync_mode(
      &state.db.conn,
      &principal.tenant_id,
      &req.content_hash,
      mode,
    )
    .await?;
  }
  Ok(Json(proto::UpsertBookResponse { content_hash: req.content_hash }))
}

/// `PUT /api/v1/books/{content_hash}/blob` — upload the document bytes.
pub async fn put_blob(
  SyncPrincipal(principal): SyncPrincipal,
  State(state): State<AppState>,
  Path(content_hash): Path<String>,
  body: Bytes,
) -> AppResult<Json<proto::PutBlobResponse>> {
  if !principal.can_write_book(&content_hash) {
    return Err(AppError::Forbidden);
  }
  let access = repo::access::library_for_hash(
    &state.db.conn,
    state.entitlements.as_ref(),
    &principal.tenant_id,
    &principal.user_id,
    principal.role.is_admin(),
    principal.personal_sync,
    Some(&principal.device_id),
    &content_hash,
  )
  .await?;
  if !access.can_write() {
    return Err(AppError::Forbidden);
  }
  // Enforce the account-wide ceiling regardless of what the client believes:
  // in `metadata`/`off` the document's bytes never leave the device, so the
  // server refuses the blob even if an old or misconfigured client sends it.
  let mode =
    repo::books::sync_mode(&state.db.conn, &principal.tenant_id, &content_hash)
      .await?;
  if !mode.syncs_blob() {
    return Err(AppError::Conflict(format!(
      "document sync mode is '{mode}'; blob upload is disabled"
    )));
  }
  let book_id = repo::books::find_id_by_hash(
    &state.db.conn,
    &principal.tenant_id,
    &content_hash,
  )
  .await?
  .ok_or(AppError::NotFound)?;
  let sha256 = sha256_hex(&body);
  repo::blobs::put(
    &state.db.conn,
    &principal.tenant_id,
    &book_id,
    &body,
    &sha256,
  )
  .await?;
  // Re-evaluate storage limits after the upload (best-effort).
  if let Some(org) = repo::books::access_meta(
    &state.db.conn,
    &principal.tenant_id,
    &content_hash,
  )
  .await?
  .and_then(|meta| meta.organization_id)
  {
    crate::web::check_org(&state, &principal.tenant_id, &org).await;
  }
  crate::web::check_server_storage(&state, &principal.tenant_id).await;
  // Warm the canonical-extraction cache in the background so later imports and
  // thin clients reuse the extraction instead of re-running OCR/pandoc.
  if state.config.extraction_cache {
    crate::api::convert::spawn_prewarm_extraction(
      &state,
      &principal.tenant_id,
      &content_hash,
      body.to_vec(),
    );
  }
  Ok(Json(proto::PutBlobResponse { byte_len: body.len() as u64, sha256 }))
}

/// `GET /api/v1/books/{content_hash}/blob` — download the document bytes.
pub async fn get_blob(
  SyncPrincipal(principal): SyncPrincipal,
  State(state): State<AppState>,
  Path(content_hash): Path<String>,
) -> AppResult<Response> {
  if !principal.can_read_book(&content_hash) {
    return Err(AppError::Forbidden);
  }
  let access = repo::access::library_for_hash(
    &state.db.conn,
    state.entitlements.as_ref(),
    &principal.tenant_id,
    &principal.user_id,
    principal.role.is_admin(),
    principal.personal_sync,
    Some(&principal.device_id),
    &content_hash,
  )
  .await?;
  if !access.can_read() {
    return Err(AppError::NotFound);
  }
  let book_id = repo::books::find_id_by_hash(
    &state.db.conn,
    &principal.tenant_id,
    &content_hash,
  )
  .await?
  .ok_or(AppError::NotFound)?;
  let bytes = repo::blobs::get(&state.db.conn, &principal.tenant_id, &book_id)
    .await?
    .ok_or(AppError::NotFound)?;
  Ok(
    ([(header::CONTENT_TYPE, "application/octet-stream")], bytes)
      .into_response(),
  )
}

/// `PUT /api/v1/books/{content_hash}/sync-mode` — set the account-wide sync
/// ceiling for a document. Requires write access; the value is authoritative
/// for every device on the account, which each clamp their local preference
/// against it.
pub async fn set_book_sync_mode(
  SyncPrincipal(principal): SyncPrincipal,
  State(state): State<AppState>,
  Path(content_hash): Path<String>,
  Json(req): Json<proto::SetSyncModeRequest>,
) -> AppResult<Json<proto::SetSyncModeRequest>> {
  if !principal.can_write_book(&content_hash) {
    return Err(AppError::Forbidden);
  }
  let access = repo::access::library_for_hash(
    &state.db.conn,
    state.entitlements.as_ref(),
    &principal.tenant_id,
    &principal.user_id,
    principal.role.is_admin(),
    principal.personal_sync,
    Some(&principal.device_id),
    &content_hash,
  )
  .await?;
  if !access.can_write() {
    return Err(AppError::Forbidden);
  }
  let updated = repo::books::set_sync_mode(
    &state.db.conn,
    &principal.tenant_id,
    &content_hash,
    req.sync_mode,
  )
  .await?;
  if !updated {
    return Err(AppError::NotFound);
  }
  Ok(Json(proto::SetSyncModeRequest { sync_mode: req.sync_mode }))
}

fn sha256_hex(bytes: &[u8]) -> String {
  Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}
