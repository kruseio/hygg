//! Which documents sync *automatically* — the account/device-wide sync
//! **scope**, orthogonal to [`SyncMode`](super::SyncMode) (which decides *what
//! content* syncs for a document that participates at all).
//!
//! A document's outbound sync is gated by three independent layers, checked in
//! order:
//! 1. A master on/off kill switch (each client keeps this in its own config;
//!    `false` = fully serverless). Not modelled here — it gates whether any
//!    sync runs at all.
//! 2. This [`AutoSyncPolicy`] — the automatic-sync scope, combined with the
//!    per-document opt-in flag and the "looks like a book" signal by
//!    [`should_auto_sync`].
//! 3. The per-document [`SyncMode`](super::SyncMode) content granularity.
//!
//! Manual/explicit sync (the reader's `:sync`, a "Sync now" button) bypasses
//! layer 2 only — it still honours the master switch and the document's
//! `SyncMode`.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// The automatic-sync scope. Ordered least- to most-inclusive:
/// `Manual < Books < All`. The default is [`AutoSyncPolicy::Books`], so
/// book-like documents sync automatically while one-off reports and scratch
/// text stay on the device until explicitly opted in.
#[derive(
  Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "lowercase")]
pub enum AutoSyncPolicy {
  /// Only documents the user has explicitly opted in sync automatically — the
  /// "semi-automatic" mode. Manual sync still works on any document.
  Manual,
  /// Documents that look like a book (see [`looks_like_book`]) sync
  /// automatically, plus any explicit per-document opt-ins. The default.
  #[default]
  Books,
  /// Every document syncs automatically (the historical all-in behavior).
  All,
}

impl AutoSyncPolicy {
  /// The wire / config token: `manual` | `books` | `all`.
  pub fn as_str(self) -> &'static str {
    match self {
      AutoSyncPolicy::Manual => "manual",
      AutoSyncPolicy::Books => "books",
      AutoSyncPolicy::All => "all",
    }
  }

  /// Parse a stored/wire/config token, falling back to the default
  /// ([`AutoSyncPolicy::Books`]) for anything unrecognized so a bad value never
  /// silently disables or over-enables sync. Also accepts the legacy boolean
  /// `AUTO_SYNC` values so older configs migrate: `true` → `books` (the new
  /// default) and `false` → `manual` (no automatic sync, still opt-in-able).
  pub fn from_token_or_default(s: &str) -> AutoSyncPolicy {
    s.parse().unwrap_or_default()
  }
}

impl fmt::Display for AutoSyncPolicy {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(self.as_str())
  }
}

impl FromStr for AutoSyncPolicy {
  type Err = ();

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s.trim().to_ascii_lowercase().as_str() {
      "all" | "everything" | "full" => Ok(AutoSyncPolicy::All),
      // Legacy `AUTO_SYNC=true` migrates to the new book-only default.
      "books" | "book" | "true" => Ok(AutoSyncPolicy::Books),
      // Legacy `AUTO_SYNC=false` becomes opt-in-only rather than fully off;
      // the master switch is the true kill switch.
      "manual" | "optin" | "opt-in" | "off" | "false" | "none" => {
        Ok(AutoSyncPolicy::Manual)
      }
      _ => Err(()),
    }
  }
}

/// Whether a document should sync *automatically* under `policy`, given whether
/// the user has opted this specific document in and whether it looks like a
/// book. This is only the automatic-sync gate; the caller applies the master
/// switch and the per-document [`SyncMode`](super::SyncMode) around it.
pub fn should_auto_sync(
  policy: AutoSyncPolicy,
  opted_in: bool,
  looks_like_book: bool,
) -> bool {
  match policy {
    AutoSyncPolicy::All => true,
    AutoSyncPolicy::Books => opted_in || looks_like_book,
    AutoSyncPolicy::Manual => opted_in,
  }
}

/// PDFs with at least this many pages are treated as books; shorter PDFs (a
/// report, a paper, a slide deck) are not.
pub const BOOK_MIN_PDF_PAGES: u32 = 40;

/// Reflowable / plain-text documents with at least this many wrapped lines are
/// treated as books (~30–40 printed pages); shorter ones (a note, an article,
/// a memo) are not.
pub const BOOK_MIN_TEXT_LINES: usize = 1500;

/// Dedicated ebook container formats — always a book by construction.
const EBOOK_FORMATS: &[&str] = &["epub", "mobi", "azw", "azw3", "fb2", "kepub"];

/// Whether a document looks like a book, from cheap signals every client
/// already has. Deliberately conservative: an ebook container is always a book;
/// otherwise the document's length has to clear a threshold. Tunable via
/// [`BOOK_MIN_PDF_PAGES`] / [`BOOK_MIN_TEXT_LINES`].
///
/// - `format`: lowercase extension / kind (`epub`, `pdf`, `txt`, `text`, …).
/// - `total_lines`: wrapped line count of the whole document (0 if unknown).
/// - `page_count`: page count for paginated formats (PDFs); `None` otherwise.
pub fn looks_like_book(
  format: &str,
  total_lines: usize,
  page_count: Option<u32>,
) -> bool {
  let fmt = format.trim().to_ascii_lowercase();
  if EBOOK_FORMATS.contains(&fmt.as_str()) {
    return true;
  }
  // A paginated document (PDF) with a known page count: book-length pages make
  // it a book, and a present-but-short count is a strong "not a book" signal —
  // don't fall through to the line heuristic (an OCR'd short report can have
  // many text lines).
  if let Some(pages) = page_count {
    return pages >= BOOK_MIN_PDF_PAGES;
  }
  // Everything else (plain text, markdown, an unpaginated import, or a PDF
  // whose page count isn't known here): long-form content clears the line
  // threshold.
  total_lines >= BOOK_MIN_TEXT_LINES
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn default_is_books() {
    assert_eq!(AutoSyncPolicy::default(), AutoSyncPolicy::Books);
  }

  #[test]
  fn token_round_trips() {
    for p in
      [AutoSyncPolicy::Manual, AutoSyncPolicy::Books, AutoSyncPolicy::All]
    {
      assert_eq!(p.as_str().parse::<AutoSyncPolicy>(), Ok(p));
    }
  }

  #[test]
  fn legacy_bool_tokens_migrate() {
    // `AUTO_SYNC=true` (sync everything) migrates to the book-only default.
    assert_eq!("true".parse(), Ok(AutoSyncPolicy::Books));
    // `AUTO_SYNC=false` (no sync) becomes opt-in-only.
    assert_eq!("false".parse(), Ok(AutoSyncPolicy::Manual));
    assert_eq!(" OFF ".parse(), Ok(AutoSyncPolicy::Manual));
    assert_eq!(
      AutoSyncPolicy::from_token_or_default("nonsense"),
      AutoSyncPolicy::Books
    );
  }

  #[test]
  fn serde_uses_lowercase_tokens() {
    assert_eq!(
      serde_json::to_string(&AutoSyncPolicy::Books).unwrap(),
      "\"books\""
    );
    let back: AutoSyncPolicy = serde_json::from_str("\"manual\"").unwrap();
    assert_eq!(back, AutoSyncPolicy::Manual);
  }

  #[test]
  fn should_auto_sync_matrix() {
    use AutoSyncPolicy::*;
    // All: everything, regardless of opt-in or book-ness.
    assert!(should_auto_sync(All, false, false));
    // Books: book-like OR opted in.
    assert!(should_auto_sync(Books, false, true));
    assert!(should_auto_sync(Books, true, false));
    assert!(!should_auto_sync(Books, false, false));
    // Manual: only opted in.
    assert!(should_auto_sync(Manual, true, false));
    assert!(!should_auto_sync(Manual, false, true));
  }

  #[test]
  fn ebook_formats_are_always_books() {
    for fmt in ["epub", "MOBI", "azw3", "fb2", " kepub "] {
      assert!(looks_like_book(fmt, 0, None), "{fmt} should be a book");
    }
  }

  #[test]
  fn pdf_uses_page_count_when_known() {
    assert!(looks_like_book("pdf", 10, Some(BOOK_MIN_PDF_PAGES)));
    assert!(looks_like_book("pdf", 10, Some(500)));
    // A short PDF is not a book even if OCR produced many lines.
    assert!(!looks_like_book("pdf", 100_000, Some(8)));
  }

  #[test]
  fn text_uses_line_threshold_when_unpaginated() {
    assert!(looks_like_book("txt", BOOK_MIN_TEXT_LINES, None));
    assert!(looks_like_book("text", 20_000, None));
    // A short report / note stays local.
    assert!(!looks_like_book("txt", 200, None));
    assert!(!looks_like_book("md", 300, None));
    // A PDF whose page count isn't known here falls back to line length.
    assert!(looks_like_book("pdf", 5_000, None));
    assert!(!looks_like_book("pdf", 300, None));
  }
}
