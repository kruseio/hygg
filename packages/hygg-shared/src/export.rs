//! The portable, versioned bundle for a full per-user data export/import.
//!
//! A bundle carries a user's personal library — document metadata, the document
//! bytes, tags, and every annotation (reading position, bookmarks, highlights,
//! notes) — in a self-contained JSON document. It deliberately excludes
//! anything host-specific (device tokens, which are machine-bound) and anything
//! deployment-specific (whatever rules a particular server layers on), so the
//! same bundle round-trips between any two servers in either direction.
//!
//! Lives in `hygg-shared` (MIT) alongside the sync wire contract, reusing the
//! same annotation DTOs so a book's exported annotations are byte-for-byte the
//! shapes the sync API already speaks.

use serde::{Deserialize, Serialize};

use crate::sync::proto::{BookmarkDto, HighlightDto, NoteDto, ProgressDto};

/// Bundle schema version. Bumped when the shape changes incompatibly; importers
/// reject a bundle whose version they do not understand.
pub const EXPORT_FORMAT_VERSION: u32 = 1;

/// A complete export of one user's personal library.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportBundle {
  pub format_version: u32,
  /// When the bundle was produced (epoch millis).
  pub exported_at: i64,
  pub profile: ExportProfile,
  #[serde(default)]
  pub books: Vec<ExportBook>,
}

/// The exporting user's identity, for display on import ("importing library of
/// alice@example.com"). Import binds the data to the *authenticated* caller,
/// not to this email, so a user can import their own export into a new account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportProfile {
  pub email: String,
  pub name: String,
}

/// One document and everything attached to it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportBook {
  /// The content-derived, cross-device document id.
  pub content_hash: String,
  pub title: String,
  pub author: String,
  pub format: String,
  pub size_bytes: i64,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub file_name: Option<String>,
  #[serde(default)]
  pub tags: Vec<String>,
  /// The document bytes, standard-base64 encoded. `None` when metadata was
  /// synced but the blob was never uploaded.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub blob_base64: Option<String>,
  /// The reading position for this book (at most one row per user per book).
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub progress: Option<ProgressDto>,
  #[serde(default)]
  pub bookmarks: Vec<BookmarkDto>,
  #[serde(default)]
  pub highlights: Vec<HighlightDto>,
  #[serde(default)]
  pub notes: Vec<NoteDto>,
}

/// What an import applied, returned to the caller.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportSummary {
  pub books: usize,
  pub blobs: usize,
  pub progress: usize,
  pub bookmarks: usize,
  pub highlights: usize,
  pub notes: usize,
  pub tags: usize,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn bundle_round_trips_through_json() {
    let bundle = ExportBundle {
      format_version: EXPORT_FORMAT_VERSION,
      exported_at: 42,
      profile: ExportProfile {
        email: "a@example.test".into(),
        name: "Alice".into(),
      },
      books: vec![ExportBook {
        content_hash: "hash-1".into(),
        title: "Book".into(),
        author: "Author".into(),
        format: "pdf".into(),
        size_bytes: 10,
        file_name: Some("book.pdf".into()),
        tags: vec!["fiction".into()],
        blob_base64: Some("aGk=".into()),
        progress: Some(ProgressDto {
          book_id: "hash-1".into(),
          offset_line: 5,
          total_lines: 100,
          percentage: 5.0,
          viewport_offset: None,
          cursor_y: None,
          page: None,
          line_in_page: None,
          word_offset: Some(3),
          updated_at: 7,
        }),
        bookmarks: Vec::new(),
        highlights: Vec::new(),
        notes: Vec::new(),
      }],
    };
    let text = serde_json::to_string(&bundle).unwrap();
    let back: ExportBundle = serde_json::from_str(&text).unwrap();
    assert_eq!(back.format_version, EXPORT_FORMAT_VERSION);
    assert_eq!(back.books.len(), 1);
    assert_eq!(back.books[0].content_hash, "hash-1");
    assert_eq!(back.books[0].tags, vec!["fiction".to_string()]);
    assert_eq!(back.books[0].progress.as_ref().unwrap().offset_line, 5);
  }

  #[test]
  fn minimal_bundle_deserializes_with_defaults() {
    // A bundle with only the required fields must still parse (books default to
    // empty), so older/simpler producers stay compatible.
    let text = r#"{
      "format_version": 1,
      "exported_at": 0,
      "profile": { "email": "a@b.test", "name": "A" }
    }"#;
    let bundle: ExportBundle = serde_json::from_str(text).unwrap();
    assert!(bundle.books.is_empty());
  }
}
