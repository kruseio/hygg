//! Server reconcile for the standalone `run_home` picker. On open it mirrors
//! the server library (downloading documents missing locally, so a removed book
//! reappears while it still exists on the server) and reconciles each
//! document's progress last-write-wins: a newer server position overwrites the
//! local one (re-mapped by percentage when the document paginates differently
//! here, e.g. PDFs), while a newer local position is pushed up. This is the
//! home-screen twin of the reader's on-open reconcile, so the two surfaces
//! always agree. Any failure (offline, no server, revoked token, slow network)
//! falls back to local data and leaves everything untouched.

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use hygg_shared::sync::{AutoSyncPolicy, SyncMode, book_id_for_file};
use uuid::Uuid;

use super::download::download_missing_books;
use super::render::{HomeItem, load_home_items};
use crate::library::{LibraryEntry, load_index};
use crate::progress::{Progress, load_progress, save_progress_full};
use crate::sync::{ProgressPayload, RemoteBook, ServerProgress, SyncClient};

/// Home items reconciled against the server: the library is mirrored (missing
/// documents downloaded) and each document's progress is reconciled
/// last-write-wins, persisted both ways. Falls back to the local-only list when
/// no server is configured or the server is unreachable.
pub fn reconcile_home_items() -> Vec<HomeItem> {
  let Some((server, remote_books)) = fetch_server_state() else {
    return load_home_items();
  };
  // The account-wide sync ceiling per document, so we can mirror it onto each
  // local entry and gate what syncs, before `remote_books` is moved into the
  // downloader.
  let server_modes: HashMap<String, SyncMode> = remote_books
    .iter()
    .map(|book| (book.content_hash.clone(), book.sync_mode))
    .collect();
  // Mirror the server library, so a document removed locally (or added on
  // another device) is present before we reconcile its progress. Fast
  // downloads land this session; a slow or stuck blob keeps downloading in the
  // background and appears next launch, so the screen never freezes.
  mirror_server_library(remote_books);
  // Automatic-sync scope: gates which documents push their local-newer
  // position here, mirroring the reader's per-document gate. Pulling a newer
  // server position is never gated (resume a book read on another device).
  let policy = crate::config::load_server_config().auto_sync;
  let mut push_batch: Vec<ProgressPayload> = Vec::new();
  let mut items: Vec<HomeItem> = Vec::new();
  for mut entry in load_index() {
    // Refresh the mirrored server ceiling when it changed, so the effective
    // mode is current here and for the reader on its next open.
    let book_id = entry
      .source_path
      .as_deref()
      .and_then(|path| book_id_for_file(Path::new(path)));
    if let Some(mode) = book_id.as_deref().and_then(|id| server_modes.get(id))
      && entry.server_sync_mode != Some(*mode)
    {
      entry.server_sync_mode = Some(*mode);
      let hash = entry.document_hash;
      let mode = *mode;
      crate::library::update_entry(hash, |e| e.server_sync_mode = Some(mode));
    }
    let local = load_progress(entry.document_hash).ok();
    let percentage =
      reconcile_entry(&entry, local.as_ref(), &server, policy, &mut push_batch);
    let reading_seconds =
      local.as_ref().map(|p| p.reading_time_seconds).unwrap_or(0);
    items.push(HomeItem { entry, percentage, reading_seconds });
  }
  // The pull already proved the server reachable, so pushing local-newer rows
  // is fast; still bounded so a mid-flight stall can never hang the landing
  // screen.
  push_local_progress(push_batch);
  items
}

/// Reconcile one document and return the percentage to display. Writes the
/// server position to local progress when it is newer; queues a push when the
/// local position is newer; otherwise leaves both sides untouched.
fn reconcile_entry(
  entry: &LibraryEntry,
  local: Option<&Progress>,
  server: &HashMap<String, ServerProgress>,
  policy: AutoSyncPolicy,
  push_batch: &mut Vec<ProgressPayload>,
) -> f64 {
  let local_pct = local.map(|p| p.percentage).unwrap_or(0.0);
  // `off` keeps this document entirely local: display its local progress but
  // neither push nor apply server positions. (`metadata`/`full` sync state.)
  if !entry.effective_sync_mode().syncs_state() {
    return local_pct;
  }
  let local_ts = local.map(|p| p.updated_at).unwrap_or(0);
  let book_id = entry
    .source_path
    .as_deref()
    .and_then(|path| book_id_for_file(Path::new(path)));
  let Some(remote) = book_id.as_deref().and_then(|id| server.get(id)) else {
    return local_pct;
  };
  if remote.updated_at > local_ts {
    apply_server_progress_local(entry, remote, local);
    remote.percentage
  } else if local_ts > remote.updated_at
    && local_pct > 0.0
    && entry.auto_syncs(policy)
  {
    if let (Some(id), Some(p)) = (book_id, local) {
      push_batch.push(progress_payload(&id, p));
    }
    local_pct
  } else {
    local_pct
  }
}

/// Overwrite local progress with a newer server position, re-mapping the line
/// offset onto this document's own pagination and preserving locally-tracked
/// reading time. Mirrors the reader's cross-app position mapping.
fn apply_server_progress_local(
  entry: &LibraryEntry,
  remote: &ServerProgress,
  local: Option<&Progress>,
) {
  let total_lines = local
    .map(|p| p.total_lines)
    .filter(|n| *n > 0)
    .or(Some(remote.total_lines).filter(|n| *n > 0))
    .unwrap_or(0);
  let offset = if remote.total_lines == total_lines && total_lines > 0 {
    remote.offset
  } else if remote.percentage > 0.0 && total_lines > 0 {
    ((remote.percentage / 100.0) * total_lines as f64).round() as usize
  } else {
    remote.offset
  };
  let reading_seconds = local.map(|p| p.reading_time_seconds).unwrap_or(0);
  // When the sender wrapped the document at a different width, its per-line
  // anchors don't map onto this reader's lines — a full-page figure shifts
  // everything within the page. The 1-based `page` is still exact; scale the
  // in-page offset into this reader's own line space and drop the exact
  // viewport/cursor (they only line up at matching widths).
  let cross_paginated =
    remote.total_lines != 0 && remote.total_lines != total_lines;
  let (viewport_offset, cursor_y, line_in_page) = if cross_paginated {
    // The exact word anchor (persisted below) resolves the precise line once
    // the document loads. Drop the sender's per-line viewport/cursor — they
    // only line up at matching widths — and scale the PDF in-page offset as a
    // coarse fallback for rows without a word anchor (image rows).
    let scaled = remote.line_in_page.map(|lip| {
      if remote.total_lines > 0 && total_lines > 0 {
        (lip as f64 * total_lines as f64 / remote.total_lines as f64).round()
          as usize
      } else {
        lip
      }
    });
    (None, None, scaled)
  } else {
    (remote.viewport_offset, remote.cursor_y, remote.line_in_page)
  };
  let _ = save_progress_full(
    entry.document_hash,
    // Adopting the remote position: keep its own (server-domain) timestamp, so
    // the next reconcile doesn't treat this local copy as a newer local edit.
    remote.updated_at,
    offset,
    total_lines.max(offset + 1),
    // The width-independent percent is portable, so store the sender's
    // verbatim (the re-mapped local `offset` is only a coarse resume hint
    // here).
    remote.percentage,
    viewport_offset,
    cursor_y,
    remote.page,
    line_in_page,
    // The exact word anchor is width-independent; persist it verbatim so the
    // reader resolves the precise line once the document (or its page) loads.
    remote.word_offset,
    reading_seconds,
  );
}

fn progress_payload(book_id: &str, p: &Progress) -> ProgressPayload {
  ProgressPayload {
    book_id: book_id.to_string(),
    offset: p.offset,
    total_lines: p.total_lines,
    percentage: p.percentage,
    viewport_offset: p.viewport_offset,
    cursor_y: p.cursor_y,
    page: p.page,
    line_in_page: p.line_in_page,
    word_offset: p.word_offset,
    op_id: Uuid::new_v4().to_string(),
    updated_at: p.updated_at,
  }
}

/// How long the home waits for the server before falling back to local data —
/// short so an unreachable or slow server never freezes the landing screen.
const HOME_PULL_TIMEOUT: Duration = Duration::from_secs(4);
/// Cap on the local-newer push; only runs after a successful pull (server known
/// reachable), so this is a stall guard rather than the common case.
const HOME_PUSH_TIMEOUT: Duration = Duration::from_secs(4);
/// The worker's own cap on downloading missing documents; anything not fetched
/// in time retries next launch.
const DOWNLOAD_BUDGET: Duration = Duration::from_secs(20);
/// How long the home blocks on downloads before rendering. A slow or stalled
/// blob keeps downloading on the detached worker (and shows next launch) rather
/// than freezing the landing screen.
const DOWNLOAD_WAIT: Duration = Duration::from_secs(5);

/// Download server documents missing locally on a worker thread, blocking the
/// home only briefly. Whatever registers within `DOWNLOAD_WAIT` shows this
/// session; the rest continues in the background for the next launch.
fn mirror_server_library(remote_books: Vec<RemoteBook>) {
  if remote_books.is_empty() {
    return;
  }
  let (tx, rx) = std::sync::mpsc::channel();
  std::thread::spawn(move || {
    let _ = tx.send(download_missing_books(&remote_books, DOWNLOAD_BUDGET));
  });
  let _ = rx.recv_timeout(DOWNLOAD_WAIT);
}

/// Best-effort, bounded push of local-newer positions to the server.
fn push_local_progress(batch: Vec<ProgressPayload>) {
  if batch.is_empty() {
    return;
  }
  let Some(client) =
    SyncClient::from_config(&crate::config::load_server_config())
  else {
    return;
  };
  let (tx, rx) = std::sync::mpsc::channel();
  std::thread::spawn(move || {
    let _ = tx.send(client.push_progress(&batch));
  });
  let _ = rx.recv_timeout(HOME_PUSH_TIMEOUT);
}

/// Fetch the server's reading positions (keyed by book id) and book list in one
/// bounded round-trip. `None` when sync is off, unconfigured, unreachable, or
/// slow — so the caller transparently uses local data. Runs on a worker thread
/// abandoned past the timeout (the home must render promptly), so a stuck
/// connection can't hang startup. A failed book list degrades to no downloads
/// (empty list) rather than dropping the progress reconcile.
fn fetch_server_state()
-> Option<(HashMap<String, ServerProgress>, Vec<RemoteBook>)> {
  let config = crate::config::load_server_config();
  if !config.sync_enabled {
    return None;
  }
  let client = SyncClient::from_config(&config)?;
  let (tx, rx) = std::sync::mpsc::channel();
  std::thread::spawn(move || {
    let books = client.fetch_library().unwrap_or_default();
    let _ = tx.send(client.pull(0).map(|pull| (pull.progress, books)));
  });
  match rx.recv_timeout(HOME_PULL_TIMEOUT) {
    Ok(Ok((progress, books))) => {
      let map = progress.into_iter().map(|p| (p.book_id.clone(), p)).collect();
      Some((map, books))
    }
    _ => None,
  }
}
