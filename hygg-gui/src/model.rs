//! Core data types shared across the GUI. Plain serde structs — no toolkit — so
//! they round-trip cleanly through the offline store (JSON files) and stay
//! identical to the PWA's model, so a document keeps the same stable identity
//! across every hygg surface.

use hygg_shared::sync::SyncMode;
use serde::{Deserialize, Serialize};

/// How a single rendered line should be drawn.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum LineKind {
  /// Justified prose / monospace text.
  Text,
  /// A raw-ANSI truecolor art row (from a PDF image), rendered as colored
  /// half-block spans rather than plain text.
  Ansi,
}

/// A document the user has imported, fully extracted and justified for offline
/// reading. `id` is `sha256(source_bytes)` (hex) — the same stable identity the
/// CLI, PWA and server use, so a book imported here lines up with its synced
/// twin.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Book {
  pub id: String,
  pub title: String,
  /// Source format: `txt` | `epub` | `pdf` | `other`.
  pub format: String,
  /// Justification column width the `lines` were rendered at.
  pub col: usize,
  pub lines: Vec<String>,
  pub kinds: Vec<LineKind>,
  pub size_bytes: usize,
  /// Epoch milliseconds the document was imported.
  pub added_at: f64,
  /// For PDFs: the index into `lines` where each 1-based page begins
  /// (`page_starts[0]` is 0). Empty for reflowable formats. This is the
  /// pagination-independent anchor cross-device sync uses.
  #[serde(default)]
  pub page_starts: Vec<usize>,
}

// The page/word anchor helpers below are the pagination-independent positions
// cross-device sync restores by. They are a faithful port of the PWA model and
// are consumed by the sync layer (progressive enhancement, wired per target);
// `allow(dead_code)` keeps the offline-only native build warning-clean until it
// is enabled here.
#[allow(dead_code)]
impl Book {
  /// True once this book carries PDF page provenance (so page-anchored sync
  /// works). False for reflowable formats and pre-page-tracking imports.
  pub fn has_pages(&self) -> bool {
    !self.page_starts.is_empty()
  }

  /// The 1-based PDF page and the line offset within it for a flat line index.
  /// `None` when the book has no page data.
  pub fn page_of_line(&self, line: usize) -> Option<(u32, usize)> {
    if self.page_starts.is_empty() {
      return None;
    }
    let idx = match self.page_starts.binary_search(&line) {
      Ok(i) => i,
      Err(i) => i.saturating_sub(1),
    };
    Some((idx as u32 + 1, line - self.page_starts[idx]))
  }

  /// Whether rendered line `i` is an image (ASCII-art) row, which contributes
  /// nothing to the anchor so image rendering can be toggled without moving it.
  fn is_image(&self, i: usize) -> bool {
    matches!(self.kinds.get(i), Some(LineKind::Ansi))
  }

  /// The width-independent resume anchor for `line`: non-whitespace characters
  /// before it, page-local for PDFs and global otherwise. Shared with every
  /// hygg client via [`hygg_shared::anchor`].
  pub fn word_offset_of_line(&self, line: usize) -> usize {
    let start = match self.page_of_line(line) {
      Some((pg, _)) => self.page_starts[(pg as usize - 1)],
      None => 0,
    };
    hygg_shared::anchor::offset_of_line(
      &self.lines,
      |i| self.is_image(i),
      start,
      line,
    ) as usize
  }

  /// The line holding anchor `word` — searched within `page` (1-based) for a
  /// PDF, or across the whole document otherwise. Returns an absolute line
  /// index.
  pub fn line_for_word(&self, page: Option<u32>, word: usize) -> usize {
    let (start, end) = match page {
      Some(pg) if self.has_pages() => {
        let idx = (pg.max(1) as usize - 1).min(self.page_starts.len() - 1);
        let s = self.page_starts[idx];
        let e =
          self.page_starts.get(idx + 1).copied().unwrap_or(self.lines.len());
        (s, e)
      }
      _ => (0, self.lines.len()),
    };
    hygg_shared::anchor::line_for_offset(
      &self.lines,
      |i| self.is_image(i),
      start,
      end,
      word as u64,
    )
  }

  /// The flat line index for a 1-based PDF page + line offset within it,
  /// clamped to that page's rendered height (and to the document). `None`
  /// when the book has no page data.
  pub fn line_of_page(&self, page: u32, line_in_page: usize) -> Option<usize> {
    if self.page_starts.is_empty() {
      return None;
    }
    let idx = (page.max(1) as usize - 1).min(self.page_starts.len() - 1);
    let start = self.page_starts[idx];
    let end =
      self.page_starts.get(idx + 1).copied().unwrap_or(self.lines.len());
    let page_lines = end.saturating_sub(start).max(1);
    Some(
      (start + line_in_page.min(page_lines - 1))
        .min(self.lines.len().saturating_sub(1)),
    )
  }
}

/// Lightweight library-list entry (title + progress) without the full line
/// payload, so the Home grid stays cheap to render.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BookSummary {
  pub id: String,
  pub title: String,
  pub format: String,
  pub total_lines: usize,
  pub size_bytes: usize,
  pub added_at: f64,
  /// Account-wide sync ceiling, mirrored from the server library.
  #[serde(default)]
  pub sync_mode: SyncMode,
  /// This device's local sync preference. `None` = inherit the server ceiling.
  #[serde(default)]
  pub local_sync_mode: Option<SyncMode>,
  /// This device's explicit "auto-sync this document" opt-in. Under the
  /// `books` or `manual` scope a non-book document only auto-syncs when this
  /// is set.
  #[serde(default)]
  pub auto_sync_optin: bool,
}

impl BookSummary {
  /// The effective sync mode on this device: the more restrictive of the
  /// account-wide ceiling and this device's local preference. Consumed by the
  /// sync layer (see the note on `impl Book`).
  #[allow(dead_code)]
  pub fn effective_sync_mode(&self) -> SyncMode {
    self
      .sync_mode
      .most_restrictive(self.local_sync_mode.unwrap_or(SyncMode::Full))
  }

  /// Whether this document looks like a book (format + line-count signals; no
  /// page count at the summary level, so PDFs fall back to line length).
  pub fn looks_like_book(&self) -> bool {
    hygg_shared::sync::looks_like_book(&self.format, self.total_lines, None)
  }

  /// Whether this document should sync *automatically* under `scope`, combining
  /// the scope, this device's opt-in, and the book heuristic. The master switch
  /// and effective [`SyncMode`] are applied around it by the caller.
  pub fn auto_syncs(&self, scope: hygg_shared::sync::AutoSyncPolicy) -> bool {
    hygg_shared::sync::should_auto_sync(
      scope,
      self.auto_sync_optin,
      self.looks_like_book(),
    )
  }
}

impl From<&Book> for BookSummary {
  fn from(b: &Book) -> Self {
    BookSummary {
      id: b.id.clone(),
      title: b.title.clone(),
      format: b.format.clone(),
      total_lines: b.lines.len(),
      size_bytes: b.size_bytes,
      added_at: b.added_at,
      sync_mode: SyncMode::Full,
      local_sync_mode: None,
      auto_sync_optin: false,
    }
  }
}

/// Per-book reading position and lightweight reading stats. `line` is the
/// document line held at the vertical center of the viewport (the synced /
/// restored anchor). Trailing fields are `serde(default)` so older records
/// load.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Progress {
  pub line: usize,
  pub percent: f64,
  #[serde(default)]
  pub updated_at: f64,
  #[serde(default)]
  pub seconds: f64,
}

impl Progress {
  /// A document is considered finished once within a hair of the end.
  pub const FINISHED_PERCENT: f64 = 97.0;

  pub fn started(&self) -> bool {
    self.line > 0 || self.percent > 0.0
  }

  pub fn finished(&self) -> bool {
    self.percent >= Self::FINISHED_PERCENT
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// A 10-line, 3-page book (pages start at lines 0, 3, 7).
  fn paged_book() -> Book {
    let lines: Vec<String> = (0..10).map(|i| format!("word{i} x")).collect();
    Book {
      id: "id".into(),
      title: "t".into(),
      format: "pdf".into(),
      col: 40,
      kinds: vec![LineKind::Text; lines.len()],
      lines,
      size_bytes: 0,
      added_at: 0.0,
      page_starts: vec![0, 3, 7],
    }
  }

  #[test]
  fn page_and_line_round_trip() {
    let b = paged_book();
    assert!(b.has_pages());
    // Line 5 is on page 2 (starts at 3), offset 2 within it.
    assert_eq!(b.page_of_line(5), Some((2, 2)));
    assert_eq!(b.line_of_page(2, 2), Some(5));
    // Page 1 begins at line 0; the last page clamps to the document end.
    assert_eq!(b.page_of_line(0), Some((1, 0)));
    assert_eq!(b.line_of_page(3, 99), Some(9));
  }

  #[test]
  fn word_anchor_resolves_within_a_page() {
    let b = paged_book();
    // Each "wordN x" line has 6 non-whitespace characters. Line 4 is on page 2
    // (start 3), so its page-local anchor is line 3's 6 characters.
    assert_eq!(b.word_offset_of_line(4), 6);
    // That same page-local anchor maps back to line 4.
    assert_eq!(b.line_for_word(Some(2), 6), 4);
  }

  #[test]
  fn reflowable_book_has_no_pages() {
    let mut b = paged_book();
    b.page_starts.clear();
    assert!(!b.has_pages());
    assert_eq!(b.page_of_line(5), None);
    assert_eq!(b.line_of_page(1, 0), None);
  }
}
