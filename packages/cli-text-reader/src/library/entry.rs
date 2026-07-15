use chrono::Utc;
use hygg_shared::sync::{AutoSyncPolicy, SyncMode, looks_like_book};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// One row in the local library index — enough to list a previously-read
/// document on the `:home` screen and re-open it to resume.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct LibraryEntry {
  /// Local progress/bookmark/highlight key (content- or path-derived `u64`).
  pub document_hash: u64,
  /// Stable cross-device id (sha256 of text); filled when content is known,
  /// used by the sync server in later phases.
  #[serde(default)]
  pub book_id: Option<String>,
  pub title: String,
  /// Absolute path the document was opened from, so `:home` can re-open it.
  #[serde(default)]
  pub source_path: Option<String>,
  /// Lowercase extension (`pdf`, `epub`, …) or `text`.
  pub source_kind: String,
  pub total_lines: usize,
  /// Unix epoch milliseconds of the most recent open.
  pub last_opened: i64,
  /// Soft-delete tombstone. A `removed: true` line hides the document from
  /// `:home`; re-opening it writes a fresh `removed: false` line that brings
  /// it back. Defaulted so entries written before soft-delete load
  /// unchanged.
  #[serde(default)]
  pub removed: bool,
  /// This device's local sync preference for the document. `None` = inherit
  /// the server ceiling. The effective mode is the more restrictive of this
  /// and the server ceiling, so a device can opt a document down
  /// (metadata-only or off) without affecting the account-wide policy or
  /// other devices.
  #[serde(default)]
  pub local_sync_mode: Option<SyncMode>,
  /// Last-known account-wide sync ceiling for the document, mirrored from the
  /// server library. `None` until first learned; treated as `Full`.
  #[serde(default)]
  pub server_sync_mode: Option<SyncMode>,
  /// This device's explicit "auto-sync this document" opt-in. Independent of
  /// `local_sync_mode` (which caps *what* syncs): under the `books` or
  /// `manual` scope a document that isn't book-like only auto-syncs when
  /// this is set. Defaulted so entries written before the opt-in existed
  /// load unchanged.
  #[serde(default)]
  pub auto_sync_optin: bool,
}

impl LibraryEntry {
  pub fn from_path(
    document_hash: u64,
    book_id: Option<String>,
    path: &str,
    total_lines: usize,
  ) -> Self {
    Self {
      document_hash,
      book_id,
      title: title_from_path(path),
      source_path: Some(path.to_string()),
      source_kind: kind_from_path(path),
      total_lines,
      last_opened: Utc::now().timestamp_millis(),
      removed: false,
      local_sync_mode: None,
      server_sync_mode: None,
      auto_sync_optin: false,
    }
  }

  /// The effective sync mode for this document on this device: the more
  /// restrictive of the account-wide server ceiling and this device's local
  /// preference. `Full` when neither has been set.
  pub fn effective_sync_mode(&self) -> SyncMode {
    self
      .server_sync_mode
      .unwrap_or(SyncMode::Full)
      .most_restrictive(self.local_sync_mode.unwrap_or(SyncMode::Full))
  }

  /// Whether this document looks like a book, from the signals stored on the
  /// entry (format + wrapped line count; no page count at this level, so PDFs
  /// fall back to the line-length heuristic).
  pub fn looks_like_book(&self) -> bool {
    looks_like_book(&self.source_kind, self.total_lines, None)
  }

  /// Whether this document should sync automatically under `policy`, combining
  /// the scope, this device's opt-in, and the book heuristic. Does not consider
  /// the master switch or the effective [`SyncMode`], which the caller applies
  /// around it.
  pub fn auto_syncs(&self, policy: AutoSyncPolicy) -> bool {
    hygg_shared::sync::should_auto_sync(
      policy,
      self.auto_sync_optin,
      self.looks_like_book(),
    )
  }

  /// A tombstone copy of this entry (same identity, `removed: true`), appended
  /// to the index to hide the document from `:home`.
  pub fn tombstone(&self) -> Self {
    Self {
      last_opened: Utc::now().timestamp_millis(),
      removed: true,
      ..self.clone()
    }
  }
}

/// Human-friendly title derived from a file path (its stem).
pub fn title_from_path(path: &str) -> String {
  Path::new(path)
    .file_stem()
    .and_then(|stem| stem.to_str())
    .filter(|stem| !stem.is_empty())
    .unwrap_or(path)
    .to_string()
}

/// Lowercase file extension, or `text` when there is none.
pub fn kind_from_path(path: &str) -> String {
  Path::new(path)
    .extension()
    .and_then(|ext| ext.to_str())
    .map(|ext| ext.to_lowercase())
    .unwrap_or_else(|| "text".to_string())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn title_and_kind_are_derived_from_path() {
    assert_eq!(title_from_path("/books/War and Peace.epub"), "War and Peace");
    assert_eq!(kind_from_path("/books/War and Peace.epub"), "epub");
    assert_eq!(kind_from_path("/notes/scratch"), "text");
    assert_eq!(title_from_path("/notes/scratch"), "scratch");
  }

  #[test]
  fn effective_sync_mode_is_the_more_restrictive_of_server_and_local() {
    use hygg_shared::sync::SyncMode::*;
    let mut e = LibraryEntry::from_path(1, None, "/a.txt", 1);
    // Neither set → full (historical behavior).
    assert_eq!(e.effective_sync_mode(), Full);
    // A server ceiling caps the effective mode.
    e.server_sync_mode = Some(Metadata);
    assert_eq!(e.effective_sync_mode(), Metadata);
    // A local clamp can go lower.
    e.local_sync_mode = Some(Off);
    assert_eq!(e.effective_sync_mode(), Off);
    // A local clamp can never exceed the server ceiling.
    e.local_sync_mode = Some(Full);
    assert_eq!(e.effective_sync_mode(), Metadata);
  }

  #[test]
  fn auto_syncs_combines_scope_optin_and_book_heuristic() {
    use hygg_shared::sync::AutoSyncPolicy::*;
    // A short report: not book-like.
    let mut report = LibraryEntry::from_path(1, None, "/q3-report.txt", 120);
    assert!(!report.looks_like_book());
    assert!(report.auto_syncs(All));
    assert!(!report.auto_syncs(Books));
    assert!(!report.auto_syncs(Manual));
    // Opting it in makes it sync under books/manual too.
    report.auto_sync_optin = true;
    assert!(report.auto_syncs(Books));
    assert!(report.auto_syncs(Manual));

    // An epub is always book-like, so it auto-syncs under books without opt-in.
    let book = LibraryEntry::from_path(2, None, "/novel.epub", 5);
    assert!(book.looks_like_book());
    assert!(book.auto_syncs(Books));
    assert!(!book.auto_syncs(Manual));
  }

  #[test]
  fn entry_round_trips_through_json() {
    let entry =
      LibraryEntry::from_path(42, Some("abc".to_string()), "/a/b.pdf", 10);
    let json = serde_json::to_string(&entry).unwrap();
    let restored: LibraryEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(entry, restored);
    assert_eq!(restored.source_kind, "pdf");
  }
}
