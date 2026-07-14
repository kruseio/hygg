//! PDF-anchored tests for the inbound sync glue in `sync_poll`. Split out from
//! `tests_basic` to keep each file within the repository's per-file line
//! budget.

use super::*;

fn progress(offset: usize, updated_at: i64) -> crate::sync::ServerProgress {
  crate::sync::ServerProgress {
    book_id: "doc".to_string(),
    offset,
    total_lines: 0,
    percentage: 0.0,
    viewport_offset: None,
    cursor_y: None,
    page: None,
    line_in_page: None,
    word_offset: None,
    updated_at,
  }
}

#[test]
fn synced_percentage_jump_on_pdf_maps_by_character_fraction() {
  use crate::editor::streaming::{LoadedPage, PageSlot, PdfStreamingState};
  use cli_pdf_to_text::PdfStream;

  let pdf_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../test-data/pdf/progit-1-50.pdf");
  if !pdf_path.exists() {
    return;
  }
  let stream = std::sync::Arc::new(
    PdfStream::open(pdf_path.to_str().expect("utf-8 path"))
      .expect("open test pdf"),
  );
  let total_pages = stream.total_pages();
  let col = 80usize;

  // Fully loaded so `total_lines` and the per-page counts are final.
  let pages: Vec<PageSlot> = (1..=total_pages)
    .map(|p| {
      let raw = stream.extract_page(p).unwrap_or_default();
      PageSlot::Loaded(LoadedPage::from_raw(raw, col))
    })
    .collect();
  let (_tx, rx) = std::sync::mpsc::channel();
  let state = PdfStreamingState {
    stream,
    col,
    pages,
    receiver: rx,
    cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    fully_loaded: true,
    ocr_loading: false,
    ocr_receiver: None,
    ocr_cancel: None,
    ocr_worker: None,
    worker: None,
  };

  let mut editor = Editor::new(vec![String::new()], col);
  editor.height = 24;
  editor.pdf_streaming = Some(state);
  editor.rebuild_lines_from_pdf_stream();
  editor.last_local_progress_updated_at = Some(1_000);
  editor.startup_progress_reconcile = true;
  editor.offset = 0;
  editor.cursor_y = 0;

  let total_lines = editor.total_lines;
  assert!(total_lines > 0);

  // A cross-paginated position: only a percentage, no page anchor (what the
  // PWA sends for PDFs). The flat `offset` belongs to the other device's line
  // space (bogus here) and must be ignored in favour of the percentage. The
  // percentage is the width-independent character fraction, so the jump must
  // land where half the document's characters have been read — not half its
  // lines (which differs when pages vary in height).
  let mut p = progress(99_999, 2_000);
  p.total_lines = total_lines * 2; // a different pagination
  p.percentage = 50.0;
  editor.handle_server_progress(p);

  let landed = editor.offset + editor.cursor_y;
  let by_chars = crate::word_anchor::line_for_fraction(
    &editor.lines,
    &editor.line_kinds,
    0.5,
  );
  // Within one page's worth of the character-proportional target.
  let tolerance = total_lines / total_pages.max(1) + 2;
  assert!(
    landed.abs_diff(by_chars) <= tolerance,
    "percentage jump landed at {landed}, expected ~{by_chars} (±{tolerance})"
  );
}

#[test]
fn synced_jump_uses_page_anchor_while_pages_stream() {
  use crate::editor::streaming::{LoadedPage, PageSlot, PdfStreamingState};
  use cli_pdf_to_text::PdfStream;

  let pdf_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../test-data/pdf/progit-1-50.pdf");
  if !pdf_path.exists() {
    return;
  }
  let stream = std::sync::Arc::new(
    PdfStream::open(pdf_path.to_str().expect("utf-8 path"))
      .expect("open test pdf"),
  );
  let total_pages = stream.total_pages();
  assert!(total_pages >= 4, "test pdf should have several pages");
  let col = 80usize;

  // Only the target page is loaded; every other page is still a placeholder —
  // the partial-load window that used to mis-place a synced jump.
  let target_index = 2usize; // page 3
  let mut pages: Vec<PageSlot> =
    (0..total_pages).map(|_| PageSlot::Loading).collect();
  let raw = stream.extract_page(target_index + 1).unwrap_or_default();
  pages[target_index] = PageSlot::Loaded(LoadedPage::from_raw(raw, col));

  let (_tx, rx) = std::sync::mpsc::channel();
  let state = PdfStreamingState {
    stream,
    col,
    pages,
    receiver: rx,
    cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    fully_loaded: false,
    ocr_loading: false,
    ocr_receiver: None,
    ocr_cancel: None,
    ocr_worker: None,
    worker: None,
  };

  let mut editor = Editor::new(vec![String::new()], col);
  editor.height = 24;
  editor.pdf_streaming = Some(state);
  editor.rebuild_lines_from_pdf_stream();
  editor.last_local_progress_updated_at = Some(1_000);
  editor.startup_progress_reconcile = true;
  editor.offset = 0;
  editor.cursor_y = 0;

  let target_page = (target_index + 1) as u32;
  let expected_line =
    editor.pdf_line_for_page_position(target_page, 1).expect("page line");

  // A bogus flat offset (what a fully-loaded device saved) plus the stable
  // (page, line_in_page). The jump must honour the page anchor, ignoring the
  // flat offset that doesn't fit the partially-loaded buffer.
  let mut p = progress(99_999, 2_000);
  p.page = Some(target_page);
  p.line_in_page = Some(1);
  p.cursor_y = Some(0); // deterministic landing row
  editor.handle_server_progress(p);

  assert!(editor.pending_server_progress.is_none(), "should have applied");
  assert_eq!(editor.offset + editor.cursor_y, expected_line);
  assert_eq!(editor.cursor_y, 0);
}
