//! Resume-position regression tests for streamed PDFs: the saved row / word
//! anchor must survive an all-placeholder install (bundled-OCR opens preload
//! nothing) and land exactly once the target page streams in. Split out from
//! `tests` to keep each file within the repository's per-file line budget.

use super::*;
use cli_pdf_to_text::{PdfRenderedPage, PdfStream};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, mpsc};

fn text_page(stream: &PdfStream, p: usize) -> PdfRenderedPage {
  PdfRenderedPage {
    raw_text: stream.extract_page(p).unwrap_or_default(),
    lines: vec![],
    line_kinds: vec![],
    contains_images: false,
  }
}

/// Editor over the test PDF with EVERY page a placeholder (preload radius 0 —
/// the bundled-OCR install state), plus the loader-side page sender and the
/// shared stream. `None` when the test PDF is absent.
fn placeholder_pdf_editor()
-> Option<(crate::editor::Editor, mpsc::SyncSender<PageLoaded>, Arc<PdfStream>)>
{
  let pdf_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../test-data/pdf/progit-1-50.pdf");
  if !pdf_path.exists() {
    return None;
  }
  let stream = Arc::new(PdfStream::open(pdf_path.to_str().unwrap()).unwrap());
  let total = stream.total_pages();
  let col = 80usize;
  assert!(total > 20);
  let (tx, rx) = mpsc::sync_channel(4096);
  let state = PdfStreamingState {
    stream: Arc::clone(&stream),
    col,
    pages: (0..total).map(|_| PageSlot::Loading).collect(),
    receiver: rx,
    cancel: Arc::new(AtomicBool::new(false)),
    fully_loaded: false,
    ocr_loading: false,
    ocr_receiver: None,
    ocr_cancel: None,
    ocr_worker: None,
    worker: None,
  };
  let mut editor = crate::editor::Editor::new(vec![String::new()], col);
  editor.col = col;
  editor.height = 41;
  editor.pdf_streaming = Some(state);
  editor.rebuild_lines_from_pdf_stream();
  Some((editor, tx, stream))
}

/// Stream every page in the loader's block order (target first), draining a
/// block at a time then centering — what the render tick does.
fn stream_all_pages(
  editor: &mut crate::editor::Editor,
  tx: &mpsc::SyncSender<PageLoaded>,
  stream: &PdfStream,
  target_page: usize,
) {
  let total = stream.total_pages();
  let mut order: Vec<usize> = vec![target_page];
  order.extend(crate::editor::streaming_loader::load_order(target_page, total));
  for chunk in order.chunks(10) {
    for &p in chunk {
      tx.send(PageLoaded::Page {
        page_index: p - 1,
        rendered_page: text_page(stream, p),
        replace_existing: false,
      })
      .unwrap();
    }
    editor.drain_pdf_stream();
    editor.center_cursor();
  }
}

/// Regression for the resume bug: when the saved page hasn't preloaded (a
/// placeholder at install — e.g. bundled-OCR opens with preload radius 0), the
/// saved `line_in_page` must not be clamped against the placeholder's 1-line
/// height and lost. The position is held in `pdf_restore_target` and applied
/// exactly once the page streams in.
#[test]
fn resume_lands_on_saved_line_when_target_page_streams_in_late() {
  let Some((mut editor, tx, stream)) = placeholder_pdf_editor() else {
    return;
  };
  let target_page = 14usize;
  let saved_lip = 12usize; // a row well past the placeholder's single line

  // The target page is a placeholder, so the saved row would clamp to 0 if
  // applied now. Holding it in pdf_restore_target keeps it intact.
  editor.pdf_restore_target = Some(crate::core_state::PdfRestoreTarget {
    page: target_page as u32,
    line_in_page: saved_lip,
    cursor_y: None,
    word_offset: None,
  });
  editor.apply_pdf_restore_target_if_ready();
  assert!(
    editor.pdf_restore_target.is_some(),
    "must stay pending while the target page is a placeholder"
  );

  stream_all_pages(&mut editor, &tx, &stream, target_page);

  // Landed on the exact saved (page, line_in_page), not the clamped placeholder
  // row, and stayed there through the rest of the load.
  assert!(editor.pdf_restore_target.is_none(), "target should be applied");
  assert_eq!(
    editor.current_pdf_position(),
    Some((target_page as u32, saved_lip)),
    "resume must land on the saved row once the page streams in"
  );
}

/// Regression for the word-anchor variant of the same bug (the "reopen landed
/// 18 lines back at 19%" report): the exact resume anchor must not be resolved
/// against a placeholder's own loading-message characters — that clamps to the
/// placeholder's last row, restores the page *start* instead of the saved row,
/// and then sticks there for the whole session. The anchor stays unresolved in
/// `pdf_restore_target` until the page (and its seam neighbours) have real
/// content, then lands on the exact saved row.
#[test]
fn resume_word_anchor_not_resolved_against_placeholder() {
  let target_page = 14usize;
  let saved_lip = 12usize;

  // Ground truth: the page-local anchor of (page, row) measured against the
  // fully-loaded flat buffer — what the exit snapshot persisted.
  let Some((mut reference, rtx, rstream)) = placeholder_pdf_editor() else {
    return;
  };
  stream_all_pages(&mut reference, &rtx, &rstream, target_page);
  let start = reference
    .pdf_streaming
    .as_ref()
    .unwrap()
    .line_start_for_page(target_page - 1);
  let word = crate::word_anchor::words_in_range(
    &reference.lines,
    &reference.line_kinds,
    start,
    start + saved_lip,
  );
  assert!(word > 0, "row {saved_lip} should sit past real page content");

  let (mut editor, tx, stream) = placeholder_pdf_editor().unwrap();
  editor.pdf_restore_target = Some(crate::core_state::PdfRestoreTarget {
    page: target_page as u32,
    line_in_page: saved_lip,
    cursor_y: None,
    word_offset: Some(word),
  });
  editor.apply_pdf_restore_target_if_ready();
  assert!(
    editor.pdf_restore_target.is_some(),
    "must stay pending while the target page is a placeholder"
  );

  stream_all_pages(&mut editor, &tx, &stream, target_page);

  assert!(editor.pdf_restore_target.is_none(), "target should be applied");
  assert_eq!(
    editor.current_pdf_position(),
    Some((target_page as u32, saved_lip)),
    "the word anchor must resolve to the exact saved row, not the page start"
  );
}

/// End-to-end regression through the real open path
/// (`poll_pending_pdf_stream`): a bundled-OCR open preloads *nothing*, so at
/// install every page is a placeholder. The install must carry the saved word
/// anchor forward unresolved — resolving it there against the placeholder is
/// exactly the bug that restored the page start (18 lines / one screen above
/// the saved row) on every reopen with PDF_OCR=true.
#[test]
fn open_with_no_preloaded_pages_resumes_on_exact_saved_row() {
  let target_page = 14usize;
  let saved_lip = 12usize;

  // Ground truth anchor, as the exit snapshot would have persisted it.
  let Some((mut reference, rtx, rstream)) = placeholder_pdf_editor() else {
    return;
  };
  stream_all_pages(&mut reference, &rtx, &rstream, target_page);
  let start = reference
    .pdf_streaming
    .as_ref()
    .unwrap()
    .line_start_for_page(target_page - 1);
  let word = crate::word_anchor::words_in_range(
    &reference.lines,
    &reference.line_kinds,
    start,
    start + saved_lip,
  );

  // The opener thread's message for a bundled-OCR open: no preloaded pages.
  let (ready_tx, ready_rx) = mpsc::channel();
  let (page_tx, page_rx) = mpsc::sync_channel(4096);
  ready_tx
    .send(StreamReady::Ok {
      stream: Arc::clone(&rstream),
      target_page,
      restore_line_in_page: Some(saved_lip),
      preloaded_pages: Vec::new(),
      pages_receiver: page_rx,
      cancel: Arc::new(AtomicBool::new(false)),
      worker: std::thread::spawn(|| {}),
      ocr_loading: false,
    })
    .unwrap();

  let col = 80usize;
  let mut editor = crate::editor::Editor::new(vec![String::new()], col);
  editor.col = col;
  editor.height = 41;
  editor.pdf_pending = Some(PendingPdfStream {
    receiver: ready_rx,
    started_at: std::time::Instant::now(),
    canonical_path_display: "progit-1-50.pdf".to_string(),
    restore_line_in_page: Some(saved_lip),
    restore_cursor_y: Some(19),
    restore_word_offset: Some(word),
  });
  assert!(editor.poll_pending_pdf_stream(), "install should complete");

  stream_all_pages(&mut editor, &page_tx, &rstream, target_page);

  assert_eq!(
    editor.current_pdf_position(),
    Some((target_page as u32, saved_lip)),
    "reopen must land on the saved row, not the placeholder-resolved page start"
  );
}
