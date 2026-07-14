//! Saved-position restoration for streamed PDFs: load the persisted reading
//! position and, when only a flat line offset was stored, infer which page +
//! line it lands on. Kept separate from the open/run glue in `pdf.rs`.

use crate::progress::load_progress;

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct SavedPdfPosition {
  pub(super) target_page: Option<usize>,
  pub(super) line_in_page: Option<usize>,
  pub(super) cursor_y: Option<usize>,
  /// Page-local non-whitespace character offset of the saved line — the
  /// width-independent anchor used to resolve the exact line within the page.
  pub(super) word_offset: Option<usize>,
  pub(super) flat_offset: Option<usize>,
  /// Save timestamp (Unix millis) of the restored local progress. Seeds
  /// `last_local_progress_updated_at` so the sync layer can tell whether a
  /// server position is genuinely newer than where we just resumed.
  pub(super) updated_at: i64,
  /// Cumulative active reading time so far (seconds); seeds the editor so time
  /// accrues across sessions.
  pub(super) reading_time_seconds: u64,
}

impl SavedPdfPosition {
  fn from_progress(progress: crate::progress::Progress) -> Self {
    let target_page = progress.page.map(|page| page as usize);
    Self {
      target_page,
      line_in_page: progress.line_in_page,
      cursor_y: progress.cursor_y,
      word_offset: progress.word_offset,
      flat_offset: target_page.is_none().then_some(progress.offset),
      updated_at: progress.updated_at,
      reading_time_seconds: progress.reading_time_seconds,
    }
  }
}

pub(super) fn load_saved_pdf_position(document_hash: u64) -> SavedPdfPosition {
  load_progress(document_hash)
    .map(SavedPdfPosition::from_progress)
    .unwrap_or_default()
}

pub(crate) fn infer_pdf_position_from_flat_offset(
  stream: &cli_pdf_to_text::PdfStream,
  flat_offset: usize,
  col: usize,
) -> Option<(usize, usize)> {
  use crate::editor::streaming::LoadedPage;
  let total_pages = stream.total_pages();
  if total_pages == 0 {
    return None;
  }

  // Load each page exactly as the streaming render does (images included).
  // A flat offset lives in the render's line space — `flat_lines()`, which is
  // what `total_lines` / `percentage` are measured against — so per-page line
  // counts must match that space. A text-only `from_raw` count omits image /
  // ASCII-art rows (e.g. a full-page cover), so a flat offset synced from a
  // reader that sends no page anchor (the PWA for PDFs) would land pages away.
  let load = |page: usize| -> Option<LoadedPage> {
    stream
      .extract_page_with_images(page, col)
      .map(|rendered| LoadedPage::from_rendered(rendered, col))
  };

  // Walk pages summing `rendered_line_count` — the same per-page total
  // `line_start_for_page` sums — keeping a prev/next window so seam stitching
  // and the inter-page separator are counted identically to `flat_lines()`.
  let mut remaining = flat_offset;
  let mut prev: Option<LoadedPage> = None;
  let mut current = load(1);
  for page in 1..=total_pages {
    let next = if page < total_pages { load(page + 1) } else { None };
    let page_lines = current
      .as_ref()
      .map(|p| p.rendered_line_count(prev.as_ref(), next.as_ref(), false, col))
      .unwrap_or(1)
      .max(1);
    if remaining < page_lines {
      return Some((page, remaining));
    }
    remaining = remaining.saturating_sub(page_lines);
    prev = current;
    current = next;
  }

  Some((total_pages, 0))
}

#[cfg(test)]
mod tests {
  use super::*;

  fn progress_at(page: u32, updated_at: i64) -> crate::progress::Progress {
    crate::progress::Progress {
      document_hash: 1,
      updated_at,
      offset: 237,
      total_lines: 2399,
      percentage: 9.9,
      viewport_offset: Some(219),
      cursor_y: Some(18),
      page: Some(page),
      line_in_page: Some(0),
      word_offset: None,
      reading_time_seconds: 0,
    }
  }

  #[test]
  fn saved_pdf_position_carries_progress_timestamp() {
    // The restored timestamp must survive into `last_local_progress_updated_at`
    // so the sync layer can tell whether a server row is genuinely newer.
    let saved = SavedPdfPosition::from_progress(progress_at(8, 1_700));
    assert_eq!(saved.updated_at, 1_700);
    assert_eq!(saved.target_page, Some(8));
  }
}
