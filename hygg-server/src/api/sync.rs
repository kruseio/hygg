//! Batch sync endpoints. `push` applies a batch of client ops (idempotent via
//! `op_id`, last-write-wins via `updated_at`); `pull` returns rows changed
//! since a cursor. Authorization is evaluated per document from the device
//! default access and per-document overrides. Every request/response is a
//! shared `proto` DTO, so the wire contract is checked against the client at
//! compile time.

use std::collections::HashSet;

use axum::Json;
use axum::extract::{Query, State};
use hygg_shared::sync::proto::{
  self, OpPayload, PullQuery, PullResponse, PushRequest, PushResponse, SyncOp,
};

use crate::api::sync_inputs::{
  bookmark_input, highlight_input, note_input, progress_input,
  reading_day_input, reading_time_input,
};
use crate::auth::Principal;
use crate::error::AppResult;
use crate::middleware::entitlement::SyncPrincipal;
use crate::repo;
use crate::state::AppState;
use crate::util::now_millis;

/// `POST /api/v1/sync/push`
pub async fn push(
  SyncPrincipal(principal): SyncPrincipal,
  State(state): State<AppState>,
  Json(req): Json<PushRequest>,
) -> AppResult<Json<PushResponse>> {
  let pool = &state.db.conn;
  let mut applied = Vec::new();
  let mut skipped = Vec::new();

  for op in req.ops {
    if repo::ops::was_applied(pool, &principal.tenant_id, &op.op_id).await? {
      skipped.push(op.op_id);
      continue;
    }
    if apply_op(&state, &principal, &op).await? {
      repo::ops::mark_applied(pool, &principal.tenant_id, &op.op_id).await?;
      applied.push(op.op_id);
    } else {
      skipped.push(op.op_id);
    }
  }

  let server_time = now_millis();
  // Wake the user's other devices to pull (best-effort; no-op if none listen).
  if !applied.is_empty() {
    state.events.publish(&principal.tenant_id, &principal.user_id, server_time);
  }
  Ok(Json(PushResponse { applied, skipped, server_time }))
}

/// Apply a single op. Returns whether it was applied (false = skipped, e.g.
/// a permission-restricted write or an empty-key op). The exhaustive match on
/// [`OpPayload`] means a new op kind cannot be added to the protocol without
/// the server being updated to handle it.
async fn apply_op(
  state: &AppState,
  principal: &Principal,
  op: &SyncOp,
) -> AppResult<bool> {
  if !principal.can_write_book(&op.book_id) {
    return Ok(false);
  }
  // The user may sync their own annotations on any personal document, but an
  // organization document they cannot read is off-limits.
  if !repo::access::annotation_readable_for_hash(
    &state.db.conn,
    state.entitlements.as_ref(),
    &principal.tenant_id,
    &principal.user_id,
    principal.role.is_admin(),
    principal.personal_sync,
    Some(&principal.device_id),
    &op.book_id,
  )
  .await?
  {
    return Ok(false);
  }
  // Enforce the account-wide ceiling: `off` means nothing about this document
  // syncs, so drop the op. `metadata`/`full` both keep reading state, so they
  // fall through. The server is authoritative here even if a client still
  // tries.
  if !repo::books::sync_mode(&state.db.conn, &principal.tenant_id, &op.book_id)
    .await?
    .syncs_state()
  {
    return Ok(false);
  }
  let pool = &state.db.conn;
  let tenant = &principal.tenant_id;
  let user = &principal.user_id;
  match &op.payload {
    OpPayload::Progress(data) => {
      let input = progress_input(principal, op, data);
      repo::progress::upsert(pool, tenant, user, &input).await?;
      Ok(true)
    }
    OpPayload::Bookmark(data) => {
      if data.mark.is_empty() {
        return Ok(false);
      }
      let input = bookmark_input(principal, op, data);
      repo::bookmarks::upsert(pool, tenant, user, &input).await?;
      Ok(true)
    }
    OpPayload::Highlight(data) => {
      let input = highlight_input(principal, op, data);
      repo::highlights::upsert(pool, tenant, user, &input).await?;
      Ok(true)
    }
    OpPayload::Note(data) => {
      if data.id.is_empty() {
        return Ok(false);
      }
      let input = note_input(principal, op, data);
      repo::notes::upsert(pool, tenant, user, &input).await?;
      Ok(true)
    }
    OpPayload::ReadingTime(data) => {
      let input = reading_time_input(principal, op, data);
      repo::reading::upsert_time(pool, tenant, user, &input).await?;
      Ok(true)
    }
    OpPayload::ReadingDay(data) => {
      if data.day.is_empty() {
        return Ok(false);
      }
      let input = reading_day_input(principal, op, data);
      repo::reading::upsert_day(pool, tenant, user, &input).await?;
      Ok(true)
    }
  }
}

/// The subset of the given book ids the principal may receive synced rows for:
/// the device must allow reads and, for organization documents, the user must
/// have read access under the permission model. Deduped to one resolution per
/// book.
async fn readable_books<'a>(
  state: &AppState,
  principal: &Principal,
  ids: impl Iterator<Item = &'a str>,
) -> AppResult<HashSet<String>> {
  let mut allowed = HashSet::new();
  let mut seen = HashSet::new();
  let is_admin = principal.role.is_admin();
  let entitled = principal.personal_sync;
  for id in ids {
    if !seen.insert(id) {
      continue;
    }
    if principal.can_read_book(id)
      && repo::access::annotation_readable_for_hash(
        &state.db.conn,
        state.entitlements.as_ref(),
        &principal.tenant_id,
        &principal.user_id,
        is_admin,
        entitled,
        Some(&principal.device_id),
        id,
      )
      .await?
    {
      allowed.insert(id.to_string());
    }
  }
  Ok(allowed)
}

/// `GET /api/v1/sync/pull?since=<millis>`
pub async fn pull(
  SyncPrincipal(principal): SyncPrincipal,
  State(state): State<AppState>,
  Query(params): Query<PullQuery>,
) -> AppResult<Json<PullResponse>> {
  let since = params.since.unwrap_or(0);
  let pool = &state.db.conn;
  let tenant = &principal.tenant_id;
  let user = &principal.user_id;
  let mut progress =
    repo::progress::list_since(pool, tenant, user, since).await?;
  let mut bookmarks =
    repo::bookmarks::list_since(pool, tenant, user, since).await?;
  let mut highlights =
    repo::highlights::list_since(pool, tenant, user, since).await?;
  let mut notes = repo::notes::list_since(pool, tenant, user, since).await?;
  let allowed = readable_books(
    &state,
    &principal,
    progress
      .iter()
      .map(|r| r.book_id.as_str())
      .chain(bookmarks.iter().map(|r| r.book_id.as_str()))
      .chain(highlights.iter().map(|r| r.book_id.as_str()))
      .chain(notes.iter().map(|r| r.book_id.as_str())),
  )
  .await?;
  progress.retain(|r| allowed.contains(&r.book_id));
  bookmarks.retain(|r| allowed.contains(&r.book_id));
  highlights.retain(|r| allowed.contains(&r.book_id));
  notes.retain(|r| allowed.contains(&r.book_id));
  Ok(Json(PullResponse {
    server_time: now_millis(),
    progress: progress.into_iter().map(Into::into).collect(),
    bookmarks: bookmarks.into_iter().map(Into::into).collect(),
    highlights: highlights.into_iter().map(Into::into).collect(),
    notes: notes.into_iter().map(Into::into).collect(),
  }))
}
