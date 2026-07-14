//! Home helpers: the combined library + progress load, the two-phase server
//! sync (metadata + progress first for an instant UI, document bytes after),
//! and the small dashboard formatters. Split out to keep `home.rs` within the
//! LOC budget.

use std::collections::{HashMap, HashSet};

use crate::model::{BookSummary, Progress};
use crate::{storage, sync};

/// The library plus each book's progress, for the home dashboard.
pub async fn load_library_and_progress()
-> (Vec<BookSummary>, HashMap<String, Progress>) {
  let library = storage::list_library().await.unwrap_or_default();
  let mut progress = HashMap::new();
  for book in &library {
    if let Ok(p) = storage::get_progress(&book.id).await {
      progress.insert(book.id.clone(), p);
    }
  }
  (library, progress)
}

/// A server document whose content still needs downloading — its metadata is
/// already stored (and shown on the home) but its bytes are not local yet.
#[derive(Clone)]
pub struct PendingBody {
  pub id: String,
  pub title: String,
  pub format: String,
}

/// Phase 1 (fast, metadata only): reconcile the library list and reading
/// positions from the server *without* downloading any document bytes. Every
/// server document missing locally gets a metadata-only library row so it shows
/// on the home immediately; the returned list is what still needs its content
/// fetched (by [`download_bodies`]). Empty on any network failure — the home
/// then just keeps its local view.
pub async fn sync_metadata(creds: &sync::Creds) -> Vec<PendingBody> {
  let Ok(remote) = sync::list_books(creds).await else {
    return Vec::new();
  };
  let local: HashSet<String> = storage::list_library()
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|b| b.id)
    .collect();
  for book in &remote {
    if local.contains(&book.content_hash) {
      // Refresh the account-wide ceiling, preserving the local clamp + rest.
      let _ = storage::update_summary(&book.content_hash, |s| {
        s.sync_mode = book.sync_mode;
      })
      .await;
    } else {
      let _ = storage::put_summary(&BookSummary {
        id: book.content_hash.clone(),
        title: book.title.clone(),
        format: book.format.clone(),
        total_lines: 0, // filled in once the content is imported
        size_bytes: book.size_bytes.max(0) as usize,
        added_at: js_sys::Date::now(),
        sync_mode: book.sync_mode,
        local_sync_mode: None,
        auto_sync_optin: false,
      })
      .await;
    }
  }
  merge_remote_progress(creds).await;
  let mut pending = Vec::new();
  for book in remote {
    // Only documents whose effective mode syncs bytes have anything to fetch;
    // metadata-only / off keep their bytes on the owning device.
    let syncs_blob = storage::get_summary(&book.content_hash)
      .await
      .map(|s| s.effective_sync_mode().syncs_blob())
      .unwrap_or(true);
    if syncs_blob && !storage::has_book(&book.content_hash).await {
      pending.push(PendingBody {
        id: book.content_hash,
        title: book.title,
        format: book.format,
      });
    }
  }
  pending
}

/// Phase 2 (background): download the bytes for `pending` documents and import
/// each into a full, openable book, so a document the user taps has very likely
/// already been fetched. Returns how many were downloaded. Best-effort — a
/// document that fails to fetch or import stays metadata-only and is fetched on
/// demand when opened.
pub async fn download_bodies(
  creds: &sync::Creds,
  col: usize,
  pending: Vec<PendingBody>,
) -> usize {
  let mut done = 0;
  for body in pending {
    if let Ok(bytes) = sync::download_blob(creds, &body.id).await {
      let filename = format!("{}.{}", body.title, body.format);
      // Client-side extraction when possible, else the server's conversion of
      // the same stored document (DOCX / scanned PDFs the browser can't read).
      // A conversion the server declines just leaves the document
      // metadata-only here; its explanation is shown when it is opened.
      if let Ok(imported) = super::import_flow::book_from_download(
        creds, &body.id, &filename, &bytes, col,
      )
      .await
        && storage::put_book(&imported, &bytes).await.is_ok()
      {
        done += 1;
      }
    }
  }
  done
}

/// Merge remote reading positions into local progress: adopt a row only when it
/// is newer than what is stored, re-mapping by percentage when the document
/// paginates differently here (PDFs), and always keeping local reading seconds.
///
/// The re-mapped `line` is only a coarse *line-fraction* hint — the home has no
/// book lines to resolve the row's word/page anchors (or the character-fraction
/// percentage) against, and line-fraction diverges from the shared
/// character-fraction percent wherever page heights vary. The row's own
/// `updated_at` is stored with it precisely so the reader, which does load the
/// book, re-resolves the same server row exactly on open (`reader_load` adopts
/// on `>=`, so this copy never shadows it).
async fn merge_remote_progress(creds: &sync::Creds) {
  let totals: HashMap<String, usize> = storage::list_library()
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|b| (b.id, b.total_lines))
    .collect();
  let Ok(rows) = sync::pull_progress(creds, None).await else {
    return;
  };
  for dto in rows {
    let Some(&total) = totals.get(&dto.book_id) else {
      continue;
    };
    let mut p = storage::get_progress(&dto.book_id).await.unwrap_or_default();
    if (dto.updated_at as f64) <= p.updated_at {
      continue;
    }
    p.line = if dto.total_lines as usize == total && dto.offset_line >= 0 {
      dto.offset_line as usize
    } else if dto.percentage > 0.0 && total > 0 {
      ((dto.percentage / 100.0) * total as f64).round() as usize
    } else {
      dto.offset_line.max(0) as usize
    };
    p.percent = dto.percentage;
    p.updated_at = dto.updated_at as f64;
    let _ = storage::put_progress(&dto.book_id, p).await;
  }
}

/// Compact reading-time label: `0m` / `45m` / `3h 20m`.
pub fn fmt_duration(seconds: f64) -> String {
  let total = seconds.max(0.0) as u64;
  let (h, m) = (total / 3600, (total % 3600) / 60);
  if h > 0 { format!("{h}h {m}m") } else { format!("{m}m") }
}

/// Relative "last read" label from an epoch-millis timestamp; empty when unset.
pub fn fmt_relative(ms: f64) -> String {
  if ms <= 0.0 {
    return String::new();
  }
  let mins = ((js_sys::Date::now() - ms).max(0.0) / 60_000.0) as u64;
  match mins {
    0 => "just now".to_string(),
    1..=59 => format!("{mins}m ago"),
    60..=1439 => format!("{}h ago", mins / 60),
    _ => format!("{}d ago", mins / 1440),
  }
}
