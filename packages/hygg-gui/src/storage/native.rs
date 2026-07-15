//! Native offline store: a per-user data directory of JSON files (plus raw blob
//! files). Each store is a subdirectory keyed by `book_id`. Simple,
//! transparent, and trivially inspectable — `~/.local/share/hygg-gui/` on
//! Linux, `~/Library/Application Support/com.kruseio.hygg-gui/` on macOS, and
//! `%APPDATA%\kruseio\hygg-gui\data\` on Windows.

use std::path::PathBuf;

use directories::ProjectDirs;

use crate::model::{Book, BookSummary, Progress};
use crate::storage::merge_sync_settings;

const LIBRARY: &str = "library";
const BOOKS: &str = "books";
const PROGRESS: &str = "progress";
const BLOBS: &str = "blobs";

fn data_root() -> Option<PathBuf> {
  ProjectDirs::from("com", "kruseio", "hygg-gui")
    .map(|d| d.data_dir().to_path_buf())
}

/// Path to `<data>/<store>/<id>.<ext>`, creating the store directory.
fn entry_path(store: &str, id: &str, ext: &str) -> Option<PathBuf> {
  let dir = data_root()?.join(store);
  std::fs::create_dir_all(&dir).ok()?;
  // `id` is a sha256 hex string, so it never contains path separators.
  Some(dir.join(format!("{id}.{ext}")))
}

fn read_json<T: serde::de::DeserializeOwned>(
  store: &str,
  id: &str,
) -> Option<T> {
  let path = entry_path(store, id, "json")?;
  let raw = std::fs::read_to_string(path).ok()?;
  serde_json::from_str(&raw).ok()
}

fn write_json<T: serde::Serialize>(
  store: &str,
  id: &str,
  value: &T,
) -> Result<(), String> {
  let path = entry_path(store, id, "json").ok_or("no data directory")?;
  let raw = serde_json::to_string(value).map_err(|e| e.to_string())?;
  std::fs::write(path, raw).map_err(|e| e.to_string())
}

/// Persist a freshly-imported book: its summary, full lines, and source bytes.
pub async fn put_book(book: Book, source_bytes: Vec<u8>) -> Result<(), String> {
  let mut summary = BookSummary::from(&book);
  merge_sync_settings(&mut summary, read_json(LIBRARY, &book.id).as_ref());
  write_json(LIBRARY, &book.id, &summary)?;
  write_json(BOOKS, &book.id, &book)?;
  let blob = entry_path(BLOBS, &book.id, "bin").ok_or("no data directory")?;
  std::fs::write(blob, source_bytes).map_err(|e| e.to_string())?;
  Ok(())
}

/// Persist just a book's library summary (metadata), without its content.
pub async fn put_summary(summary: BookSummary) -> Result<(), String> {
  write_json(LIBRARY, &summary.id, &summary)
}

/// A single library summary (metadata) by id, if present.
pub async fn get_summary(id: String) -> Option<BookSummary> {
  read_json(LIBRARY, &id)
}

/// Whether a book's full content (rendered lines) is stored locally. Used by
/// the (per-target) sync layer to decide on-demand fetches; unused offline.
#[allow(dead_code)]
pub async fn has_book(id: String) -> bool {
  entry_path(BOOKS, &id, "json").map(|p| p.exists()).unwrap_or(false)
}

/// All library summaries, newest-imported first.
pub async fn list_library() -> Vec<BookSummary> {
  let Some(dir) = data_root().map(|d| d.join(LIBRARY)) else {
    return Vec::new();
  };
  let Ok(entries) = std::fs::read_dir(&dir) else {
    return Vec::new();
  };
  let mut out: Vec<BookSummary> = entries
    .filter_map(|e| e.ok())
    .filter_map(|e| std::fs::read_to_string(e.path()).ok())
    .filter_map(|s| serde_json::from_str(&s).ok())
    .collect();
  out.sort_by(|a, b| {
    b.added_at.partial_cmp(&a.added_at).unwrap_or(std::cmp::Ordering::Equal)
  });
  out
}

/// Load a full book (every rendered line) for the reader.
pub async fn get_book(id: String) -> Option<Book> {
  read_json(BOOKS, &id)
}

/// The original source bytes, if retained.
pub async fn get_blob(id: String) -> Option<Vec<u8>> {
  let path = entry_path(BLOBS, &id, "bin")?;
  std::fs::read(path).ok()
}

/// Remove a book and all its associated files.
pub async fn delete_book(id: String) -> Result<(), String> {
  for (store, ext) in
    [(LIBRARY, "json"), (BOOKS, "json"), (PROGRESS, "json"), (BLOBS, "bin")]
  {
    if let Some(path) = entry_path(store, &id, ext) {
      let _ = std::fs::remove_file(path);
    }
  }
  Ok(())
}

/// Reading position for a book (defaults to the start if never opened).
pub async fn get_progress(id: String) -> Progress {
  read_json(PROGRESS, &id).unwrap_or_default()
}

/// Save reading position. Best-effort; callers fire-and-forget.
pub async fn put_progress(
  id: String,
  progress: Progress,
) -> Result<(), String> {
  write_json(PROGRESS, &id, &progress)
}
