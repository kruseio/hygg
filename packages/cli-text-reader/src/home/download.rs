//! Home-screen library mirroring: download server documents that are missing
//! locally so the home lists the same library as every other device. This is
//! what makes a local removal reversible — a document deleted here reappears on
//! the next launch as long as it still exists on the server (the same
//! last-write-wins-membership model as the PWA). Downloads are best-effort and
//! time-bounded; anything not fetched within the budget is retried next launch.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use hygg_shared::normalize_file_path;
use hygg_shared::sync::book_id_for_file;

use crate::library::{LibraryEntry, load_index, record_open};
use crate::progress::generate_hash;
use crate::sync::{RemoteBook, SyncClient};

/// Download every server book not present in the local library, registering
/// each so it appears on the home screen (and can be opened to resume).
/// Returns how many were added. Bounded by `budget`; the caller only invokes
/// this once the server has proven reachable, so it will not hang — the cap
/// just limits how much a large first sync does in one launch.
pub fn download_missing_books(
  remote: &[RemoteBook],
  budget: Duration,
) -> usize {
  let Some(client) =
    SyncClient::from_config(&crate::config::load_server_config())
  else {
    return 0;
  };
  let local = local_book_ids();
  let start = Instant::now();
  let mut added = 0;
  for book in remote {
    if start.elapsed() >= budget {
      break;
    }
    // Metadata-only / off documents keep their bytes on the owning device, so
    // there is nothing to download — skip them rather than 404.
    if !book.sync_mode.syncs_blob() {
      continue;
    }
    if local.contains(&book.content_hash) {
      continue;
    }
    if download_one(&client, book) {
      added += 1;
    }
  }
  added
}

/// Content ids of every document already in the local library (live entries),
/// so a book already present is never re-downloaded.
fn local_book_ids() -> HashSet<String> {
  load_index()
    .iter()
    .filter_map(|entry| entry.source_path.as_deref())
    .filter_map(|path| book_id_for_file(Path::new(path)))
    .collect()
}

/// Download one book into the cache and register it in the library. Returns
/// whether it was added. Best-effort: any failure (network, disk, index) is
/// swallowed so one bad book never blocks the rest or the home screen.
fn download_one(client: &SyncClient, book: &RemoteBook) -> bool {
  let Ok(bytes) = client.download_book(&book.content_hash) else {
    return false;
  };
  let Ok(path) =
    cached_book_path(&book.content_hash, &book.title, &book.format)
  else {
    return false;
  };
  if std::fs::write(&path, &bytes).is_err() {
    return false;
  }
  // Canonicalize so a PDF's path-derived `document_hash` matches the one the
  // reader computes on open (keeping the home's progress keyed the same way).
  let canonical = normalize_file_path(&path.to_string_lossy())
    .map(|p| p.to_string_lossy().to_string())
    .unwrap_or_else(|_| path.to_string_lossy().to_string());
  let entry = LibraryEntry::from_path(
    generate_hash(&canonical),
    Some(book.content_hash.clone()),
    &canonical,
    0,
  );
  record_open(&entry).is_ok()
}

/// Cache path for a downloaded document: `books/<book_id>/<title>.<ext>`. The
/// per-`book_id` directory guarantees uniqueness, while the title-based
/// filename means the local library shows a readable title (via the file stem)
/// instead of a raw content hash.
fn cached_book_path(
  book_id: &str,
  title: &str,
  ext: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
  let stem = sanitize_filename(title);
  let stem = if stem.is_empty() { book_id.to_string() } else { stem };
  let name = if ext.is_empty() { stem } else { format!("{stem}.{ext}") };
  crate::utils::get_hygg_subdir_file(&format!("books/{book_id}"), &name)
}

/// Reduce a title to a safe single-component filename stem: path separators and
/// control characters become `_`, surrounding dots/space are trimmed.
fn sanitize_filename(title: &str) -> String {
  title
    .trim()
    .chars()
    .map(|c| {
      if std::path::is_separator(c) || c.is_control() || matches!(c, ':') {
        '_'
      } else {
        c
      }
    })
    .collect::<String>()
    .trim_matches('.')
    .trim()
    .to_string()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn sanitize_filename_strips_path_separators() {
    assert_eq!(sanitize_filename("progit-1-50"), "progit-1-50");
    assert_eq!(sanitize_filename("a/b:c"), "a_b_c");
    assert_eq!(sanitize_filename("  ..hidden.. "), "hidden");
    assert!(sanitize_filename("   ").is_empty());
  }

  #[test]
  fn cached_book_path_uses_title_stem_in_book_id_dir() {
    let path = cached_book_path("abc123", "My Book", "pdf").unwrap();
    assert!(path.ends_with("books/abc123/My Book.pdf"), "{path:?}");
  }

  #[test]
  fn cached_book_path_falls_back_to_book_id_when_title_empty() {
    let path = cached_book_path("abc123", "   ", "txt").unwrap();
    assert!(path.ends_with("books/abc123/abc123.txt"), "{path:?}");
  }
}
