//! Resolving the document + resume position when the reader opens. Split out of
//! `reader.rs` (which is view-heavy) so the async loading logic stays readable.
//!
//! Loads the full book from IndexedDB — downloading on demand when only its
//! metadata is local — upgrades a pre-page-tracking PDF, then picks the resume
//! line: the server's newer position (by width-independent anchor, then page,
//! then percentage) or the local one.

use hygg_shared::sync::proto::DenialBody;

use super::import_flow::DownloadError;
use super::reader_support::fetch_book;
use super::reader_support::live::server_line_for;
use crate::format::import;
use crate::model::Book;
use crate::{storage, sync};

/// The resolved open state: the book (if it loaded), the line to restore, a
/// user-facing error when it couldn't be loaded, and — when the server
/// declined to convert it — that refusal, so the view can offer its link.
pub struct Loaded {
  pub book: Option<Book>,
  pub initial_line: usize,
  pub error: Option<String>,
  pub denial: Option<DenialBody>,
  /// Timestamp (epoch ms) of the position being restored — the server row's
  /// when we adopted it, else the local one. The reader seeds its live
  /// last-write-wins baseline with this so a peer that later advances the
  /// document is recognised as newer (see `reader_support::live`).
  pub position_updated_at: f64,
}

pub async fn resolve(
  id: String,
  creds: Option<sync::Creds>,
  col: usize,
) -> Loaded {
  // Load the full book; if only its metadata is local (a background sync hasn't
  // fetched the bytes yet), download it on demand so the reader still opens.
  // A format the browser can't render falls back to server conversion, which
  // the server may decline — captured so the view can relay its wording.
  let mut denied: Option<DenialBody> = None;
  let mut fetch_err: Option<String> = None;
  let mut loaded = match storage::get_book(&id).await.ok().flatten() {
    Some(b) => Some(b),
    None => match &creds {
      Some(creds) => match fetch_book(creds, &id, col).await {
        Ok(b) => Some(b),
        Err(DownloadError::Denied(body)) => {
          denied = Some(body);
          None
        }
        Err(DownloadError::Unavailable(e)) => {
          fetch_err = Some(e);
          None
        }
      },
      None => None,
    },
  };
  // Upgrade a PDF imported before page tracking existed: re-extract from the
  // cached bytes so page-anchored restore/sync works. No network, and the
  // re-import is persisted so this happens at most once per document.
  if let Some(b) = &loaded
    && b.format == "pdf"
    && !b.has_pages()
    && let Some(bytes) = storage::get_blob(&id).await
    && let Ok(upgraded) =
      import(&format!("{}.pdf", b.title), &bytes, b.col.max(1)).await
  {
    let _ = storage::put_book(&upgraded, &bytes).await;
    loaded = Some(upgraded);
  }
  let total = loaded.as_ref().map_or(0, |b| b.lines.len());
  let local = storage::get_progress(&id).await.unwrap_or_default();
  let mut line = local.line;
  let mut position_updated_at = local.updated_at;
  // When connected, adopt the server's position when it is at least as new as
  // the local one (last-write-wins, like the CLI). The width-independent
  // mapping (word anchor > page anchor > line/percentage) is shared with the
  // live jump so both land on the same line — see
  // `reader_support::live::server_line_for`.
  //
  // Ties go to the server row on purpose: an equal timestamp means the local
  // row is a copy of that very server row — either this reader's own push or
  // the home reconcile's adoption. The home reconcile has no book lines to
  // resolve anchors against, so its adopted `line` is only a coarse
  // line-fraction hint (18% content for a 23% position on progit); re-resolving
  // the row's own anchors here lands on the exact synced content. A strictly
  // newer local row (read further offline) still wins.
  if let Some(creds) = &creds
    && let Ok(rows) = sync::pull_progress(creds, None).await
    && let Some(p) = rows.iter().find(|p| p.book_id == id)
    && (p.updated_at as f64) >= local.updated_at
  {
    line = match &loaded {
      Some(b) => server_line_for(b, p),
      // Couldn't load the book: keep the raw offset as a best effort (moot —
      // the reader shows an error rather than a position in this case).
      None => p.offset_line.max(0) as usize,
    };
    position_updated_at = p.updated_at as f64;
  }
  // Pick the most actionable message: the server's own refusal (with whatever
  // link it offered) when conversion was declined, else the specific fetch
  // failure, else "there's no server to fetch it from".
  let (error, denial) = if loaded.is_some() {
    (None, None)
  } else if let Some(body) = denied {
    (Some(body.error.clone()), Some(body))
  } else if let Some(msg) = fetch_err {
    (Some(msg), None)
  } else if creds.is_some() {
    (Some("Failed to load the document from the server.".to_string()), None)
  } else {
    (
      Some(
        "This document isn\u{2019}t downloaded. Connect a server to load it."
          .to_string(),
      ),
      None,
    )
  };
  Loaded {
    book: loaded,
    initial_line: line.min(total.saturating_sub(1)),
    error,
    denial,
    position_updated_at,
  }
}
