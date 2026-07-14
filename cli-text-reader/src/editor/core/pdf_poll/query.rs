//! Position queries against a streaming PDF session (cursor page/line, and
//! resolving saved page/word anchors to flat lines). Split out from `pdf_poll`
//! to keep each file within the repository's per-file line budget.

use super::super::{Editor, page_and_offset_for_line};

impl Editor {
  /// Return `(page_1based, line_in_page)` for the cursor's current position
  /// in a streaming PDF session, where `line_in_page` is the row within the
  /// page's rendered output. Returns None if not streaming a PDF.
  pub fn current_pdf_position(&self) -> Option<(u32, usize)> {
    let state = self.pdf_streaming.as_ref()?;
    if state.pages.is_empty() {
      return None;
    }
    let target_line = self.offset + self.cursor_y;
    let (page_idx, line_in_page) = page_and_offset_for_line(state, target_line);
    Some(((page_idx + 1) as u32, line_in_page))
  }

  pub(crate) fn current_pdf_buffer_position(&self) -> Option<(u32, usize)> {
    let anchor = self.pdf_cursor_anchor()?;
    Some(((anchor.page_index + 1) as u32, anchor.line_in_page))
  }

  /// Flat document line for a saved (1-based `page`, `line_in_page`) in the
  /// current streaming PDF, clamped to the page's rendered height. Returns None
  /// when not streaming a PDF. Stable under partial loading: unloaded pages
  /// contribute their placeholder height, so the line is valid against the
  /// current flat buffer and `drain_pdf_stream` re-anchors onto the same page
  /// as real content arrives. Used to restore a server-synced position.
  pub(crate) fn pdf_line_for_page_position(
    &self,
    page: u32,
    line_in_page: usize,
  ) -> Option<usize> {
    let state = self.pdf_streaming.as_ref()?;
    if state.pages.is_empty() {
      return None;
    }
    let page_index =
      (page as usize).saturating_sub(1).min(state.pages.len() - 1);
    let line_start = state.line_start_for_page(page_index);
    let page_lines = state.page_line_count(page_index);
    let clamped = line_in_page.min(page_lines.saturating_sub(1));
    Some(line_start + clamped)
  }

  /// The line offset within a 1-based `page` that holds page-local source
  /// `word` — the exact, width-independent restore anchor. Resolved against the
  /// page's own rendered words, so it needs only that (preloaded) page. None
  /// when not streaming a PDF.
  pub(crate) fn page_local_line_for_word(
    &self,
    page: u32,
    word: usize,
  ) -> Option<usize> {
    let state = self.pdf_streaming.as_ref()?;
    if state.pages.is_empty() {
      return None;
    }
    let page_index =
      (page as usize).saturating_sub(1).min(state.pages.len() - 1);
    let start = state.line_start_for_page(page_index);
    let end = start + state.page_line_count(page_index);
    Some(crate::word_anchor::line_for_word_in_range(
      &self.lines,
      &self.line_kinds,
      start,
      end,
      word,
    ))
  }
}
