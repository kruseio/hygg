use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use super::entry::LibraryEntry;
use crate::utils::get_hygg_config_file;

fn index_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
  get_hygg_config_file(".library.jsonl")
}

/// Append an open event to the library index. Append-only JSONL, exactly like
/// `progress.rs`; the latest line for a given `document_hash` wins on read.
pub fn record_open(
  entry: &LibraryEntry,
) -> Result<(), Box<dyn std::error::Error>> {
  let serialized = serde_json::to_string(entry)?;
  let path = index_path()?;
  let mut file = OpenOptions::new().create(true).append(true).open(path)?;
  file.write_all(serialized.as_bytes())?;
  file.write_all(b"\n")?;
  Ok(())
}

/// Append a tombstone that hides `entry`'s document from `:home`. The document
/// reappears if it is opened again (a newer `removed: false` line wins). This
/// removes only the library listing — never the user's source file.
pub fn record_remove(
  entry: &LibraryEntry,
) -> Result<(), Box<dyn std::error::Error>> {
  record_open(&entry.tombstone())
}

/// Remove a document from the home library: tombstone the index entry and, when
/// the source lives in hygg's own `books/` download cache, delete that cached
/// copy too. Never touches a user's original file or the server — the same
/// local-only scope as the PWA's delete.
pub fn remove_document(
  entry: &LibraryEntry,
) -> Result<(), Box<dyn std::error::Error>> {
  record_remove(entry)?;
  if let Some(path) = entry.source_path.as_deref()
    && is_in_books_cache(std::path::Path::new(path))
  {
    let _ = std::fs::remove_file(path);
  }
  Ok(())
}

/// Whether `path` sits inside hygg's managed `books/` download cache — the only
/// files `remove_document` is allowed to delete from disk.
fn is_in_books_cache(path: &std::path::Path) -> bool {
  crate::utils::get_hygg_config_dir()
    .map(|dir| path.starts_with(dir.join("books")))
    .unwrap_or(false)
}

/// The most recent stored entry for `document_hash`, including tombstoned ones,
/// or `None` if the document has never been opened. Unlike [`load_index`],
/// which drops tombstones and keys by live state, this returns the raw latest
/// line — used to carry per-document settings (e.g. the sync mode) forward
/// across re-opens and removals.
pub fn latest_entry(document_hash: u64) -> Option<LibraryEntry> {
  let path = index_path().ok()?;
  let file = OpenOptions::new().read(true).open(path).ok()?;
  let mut latest: Option<LibraryEntry> = None;
  for line in BufReader::new(file).lines().map_while(Result::ok) {
    if let Ok(entry) = serde_json::from_str::<LibraryEntry>(&line)
      && entry.document_hash == document_hash
    {
      latest = Some(entry);
    }
  }
  latest
}

/// Mutate a document's stored entry in place and persist the result, keeping
/// every other field. No-op (returns `None`) when the document has no prior
/// entry. Bumps `last_opened` so the updated line wins on the next fold.
pub fn update_entry(
  document_hash: u64,
  f: impl FnOnce(&mut LibraryEntry),
) -> Option<LibraryEntry> {
  let mut entry = latest_entry(document_hash)?;
  f(&mut entry);
  entry.last_opened = chrono::Utc::now().timestamp_millis();
  let _ = record_open(&entry);
  Some(entry)
}

/// Library entries, most-recently-opened first, de-duplicated by
/// `document_hash`. Never errors — a missing or unreadable index yields an
/// empty list so `:home` degrades gracefully.
pub fn load_index() -> Vec<LibraryEntry> {
  let Ok(path) = index_path() else {
    return Vec::new();
  };
  let Ok(file) = OpenOptions::new().read(true).open(path) else {
    return Vec::new();
  };
  let lines: Vec<String> =
    BufReader::new(file).lines().map_while(Result::ok).collect();
  fold_entries(&lines)
}

/// Collapse raw JSONL lines into the latest entry per `document_hash`, sorted
/// by `last_opened` descending, with soft-deleted (tombstoned) documents
/// dropped. Pure (no I/O) so it is unit-testable.
pub(crate) fn fold_entries(raw_lines: &[String]) -> Vec<LibraryEntry> {
  let mut latest: HashMap<u64, LibraryEntry> = HashMap::new();
  for line in raw_lines {
    if let Ok(entry) = serde_json::from_str::<LibraryEntry>(line) {
      latest.insert(entry.document_hash, entry);
    }
  }
  let mut entries: Vec<LibraryEntry> =
    latest.into_values().filter(|entry| !entry.removed).collect();
  entries.sort_by_key(|entry| std::cmp::Reverse(entry.last_opened));
  entries
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::library::entry::LibraryEntry;

  fn entry(hash: u64, title: &str, last_opened: i64) -> String {
    let mut e = LibraryEntry::from_path(hash, None, "/x.pdf", 1);
    e.title = title.to_string();
    e.last_opened = last_opened;
    serde_json::to_string(&e).unwrap()
  }

  #[test]
  fn fold_keeps_latest_per_hash_and_sorts_desc() {
    let lines = vec![
      entry(1, "old-one", 100),
      entry(2, "two", 200),
      entry(1, "new-one", 300), // same hash as first, newer
    ];
    let folded = fold_entries(&lines);
    assert_eq!(folded.len(), 2);
    // Sorted by last_opened desc: hash 1 (300) before hash 2 (200).
    assert_eq!(folded[0].document_hash, 1);
    assert_eq!(folded[0].title, "new-one");
    assert_eq!(folded[1].document_hash, 2);
  }

  #[test]
  fn fold_skips_malformed_lines() {
    let lines = vec!["not json".to_string(), entry(7, "ok", 1)];
    let folded = fold_entries(&lines);
    assert_eq!(folded.len(), 1);
    assert_eq!(folded[0].document_hash, 7);
  }

  fn tombstone(hash: u64, last_opened: i64) -> String {
    let mut e = LibraryEntry::from_path(hash, None, "/x.pdf", 1);
    e.removed = true;
    e.last_opened = last_opened;
    serde_json::to_string(&e).unwrap()
  }

  #[test]
  fn fold_drops_tombstoned_documents() {
    let lines =
      vec![entry(1, "keep", 10), entry(2, "gone", 20), tombstone(2, 30)];
    let folded = fold_entries(&lines);
    assert_eq!(folded.len(), 1);
    assert_eq!(folded[0].document_hash, 1);
  }

  #[test]
  fn fold_reopen_after_tombstone_restores_document() {
    let lines =
      vec![entry(2, "gone", 20), tombstone(2, 30), entry(2, "back", 40)];
    let folded = fold_entries(&lines);
    assert_eq!(folded.len(), 1);
    assert_eq!(folded[0].title, "back");
  }
}
