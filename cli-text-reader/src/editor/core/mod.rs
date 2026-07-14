pub use crate::core_state::Editor;
pub use crate::core_types::{
  BufferState, EditorMode, EditorState, RunOutcome, SplitPosition, ViewMode,
};

mod accessors;
mod constructor;
mod progress_snapshot;
pub(crate) use progress_snapshot::SnapshotReason;
mod pdf_poll;
mod pdf_stream;
mod sync_apply;
mod sync_enqueue;
mod sync_poll;
#[cfg(test)]
mod tests;

use crate::editor::streaming::PdfStreamingState;

pub(crate) const PDF_BUFFER_INDEX: usize = 0;

#[derive(Clone, Copy)]
pub(crate) struct PdfCursorAnchor {
  pub(crate) page_index: usize,
  pub(crate) line_in_page: usize,
  pub(crate) screen_row: usize,
}

/// Map a flat line index back to (page_index, line_within_page) using the
/// streaming state's current per-page rendered line counts.
pub(crate) fn page_and_offset_for_line(
  state: &PdfStreamingState,
  line: usize,
) -> (usize, usize) {
  let mut accumulated = 0usize;
  for idx in 0..state.pages.len() {
    let count = state.page_line_count(idx);
    if line < accumulated + count {
      return (idx, line - accumulated);
    }
    accumulated += count;
  }
  let last_idx = state.pages.len().saturating_sub(1);
  let last_count = state.page_line_count(last_idx);
  (last_idx, last_count.saturating_sub(1))
}

pub(crate) fn reanchored_pdf_line(
  page_counts: &[usize],
  anchor: PdfCursorAnchor,
) -> usize {
  let mut line = 0usize;
  for (idx, count) in page_counts.iter().enumerate() {
    if idx >= anchor.page_index {
      break;
    }
    line += count;
  }
  let clamped_line_in_page = anchor.line_in_page.min(
    page_counts.get(anchor.page_index).copied().unwrap_or(0).saturating_sub(1),
  );
  line + clamped_line_in_page
}

pub(crate) fn restored_pdf_viewport(
  document_line: usize,
  content_height: usize,
  restore_cursor_y: Option<usize>,
) -> (usize, usize) {
  let landing_y = restore_cursor_y
    .unwrap_or(content_height / 2)
    .min(content_height.saturating_sub(1));
  if document_line < landing_y {
    (0, document_line)
  } else {
    (document_line - landing_y, landing_y)
  }
}
