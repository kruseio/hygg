//! Core data types shared across the PWA. Plain serde structs — no DOM — so
//! they round-trip cleanly through IndexedDB (stored as JSON strings).

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
/// CLI and server use, so a book imported here lines up with its synced twin.
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
  /// (`page_starts[0]` is 0). Empty for reflowable formats and for books
  /// imported before page tracking existed. This is the pagination-independent
  /// anchor cross-device sync uses: a `(page, line_in_page)` maps to the same
  /// page in a reader that wrapped the document at a different width.
  #[serde(default)]
  pub page_starts: Vec<usize>,
}

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
    // Last page whose start is <= line (page_starts is ascending).
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
  /// before it, page-local (from the page's first line) for PDFs and global
  /// otherwise. Shared with every hygg client via [`hygg_shared::anchor`], so
  /// the same content resolves to the same anchor in any reader.
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

  /// Width-independent reading percent (`0..=100`) of a document line: the
  /// shared non-whitespace-character fraction (see [`hygg_shared::anchor`]), so
  /// the reader indicator matches what the CLI and the server show for the same
  /// content — regardless of each client's wrap width. A line-index percent
  /// would disagree (a wider column wraps into fewer lines).
  pub fn percent_of_line(&self, line: usize) -> f64 {
    hygg_shared::anchor::fraction_of_line(
      &self.lines,
      |i| self.is_image(i),
      line,
    ) * 100.0
  }

  /// The line at reading percent `percent` (`0..=100`) — the inverse of
  /// [`percent_of_line`](Self::percent_of_line), for resuming a position a
  /// differently-wrapped peer synced as a percentage.
  pub fn line_for_percent(&self, percent: f64) -> usize {
    hygg_shared::anchor::line_for_fraction(
      &self.lines,
      |i| self.is_image(i),
      percent / 100.0,
    )
  }

  /// The flat line index for a 1-based PDF page + line offset within it,
  /// clamped to that page's rendered height (and to the document). `None` when
  /// the book has no page data.
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
  /// Account-wide sync ceiling, mirrored from the server library. `Full` until
  /// first learned. `serde(default)` keeps rows written before this existed.
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
  /// account-wide ceiling and this device's local preference.
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
      // Defaults; `storage::put_book` carries any prior sync settings forward.
      sync_mode: SyncMode::Full,
      local_sync_mode: None,
      auto_sync_optin: false,
    }
  }
}

/// Per-book reading position and lightweight reading stats. `line` is the top
/// visible document line; the reader restores the scroll offset from it.
/// `updated_at` (epoch millis) powers "last read" and `seconds` accumulates
/// active reading time — both shown on the home dashboard. The trailing fields
/// are `serde(default)` so progress saved before they existed still loads.
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
  /// A document is considered finished once within a hair of the end (the last
  /// screen rarely scrolls to an exact 100%).
  pub const FINISHED_PERCENT: f64 = 97.0;

  pub fn started(&self) -> bool {
    self.line > 0 || self.percent > 0.0
  }

  pub fn finished(&self) -> bool {
    self.percent >= Self::FINISHED_PERCENT
  }
}
