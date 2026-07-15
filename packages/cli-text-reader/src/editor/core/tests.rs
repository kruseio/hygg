use super::*;
use crate::editor::streaming::{
  LoadedPage, PageLoaded, PageSlot, PdfStreamingState,
};
use cli_pdf_to_text::{PdfLineKind, PdfRenderedPage, PdfStream};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, mpsc};

fn test_pdf_stream() -> Option<Arc<PdfStream>> {
  let pdf_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../test-data/pdf/progit-1-50.pdf");
  if !pdf_path.exists() {
    return None;
  }
  Some(Arc::new(
    PdfStream::open(pdf_path.to_str().expect("utf-8 path"))
      .expect("PdfStream should open valid test PDF"),
  ))
}

fn rendered_image_page(lines: &[&str]) -> PdfRenderedPage {
  PdfRenderedPage {
    raw_text: lines.join("\n"),
    lines: lines.iter().map(|line| (*line).to_string()).collect(),
    line_kinds: vec![PdfLineKind::Text; lines.len()],
    contains_images: true,
  }
}

fn editor_with_two_page_pdf() -> Option<(Editor, mpsc::SyncSender<PageLoaded>)>
{
  let stream = test_pdf_stream()?;
  let (tx, rx) = mpsc::sync_channel(4);
  let mut editor = Editor::new(vec!["placeholder".to_string()], 80);
  editor.height = 10;
  editor.ocr_enabled = true;
  editor.pdf_streaming = Some(PdfStreamingState {
    stream,
    col: 80,
    pages: vec![
      PageSlot::Loaded(LoadedPage::from_rendered(
        rendered_image_page(&["p1-0", "p1-1", "p1-2"]),
        80,
      )),
      PageSlot::Loaded(LoadedPage::from_rendered(
        rendered_image_page(&["p2-0", "p2-1"]),
        80,
      )),
    ],
    receiver: rx,
    cancel: Arc::new(AtomicBool::new(false)),
    fully_loaded: true,
    ocr_loading: true,
    ocr_receiver: None,
    ocr_cancel: None,
    ocr_worker: None,
    worker: None,
  });
  editor.rebuild_lines_from_pdf_stream();
  Some((editor, tx))
}

#[test]
fn pdf_restore_uses_saved_cursor_screen_row() {
  assert_eq!(restored_pdf_viewport(42, 24, Some(14)), (28, 14));
}

#[test]
fn pdf_restore_clamps_saved_cursor_row_near_top() {
  assert_eq!(restored_pdf_viewport(3, 24, Some(14)), (0, 3));
}

#[test]
fn pdf_restore_falls_back_to_center_without_saved_cursor_row() {
  assert_eq!(restored_pdf_viewport(42, 24, None), (30, 12));
}

#[test]
fn drain_pdf_stream_replaces_loaded_page_for_ocr_update() {
  let pdf_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../test-data/pdf/progit-1-50.pdf");
  if !pdf_path.exists() {
    return;
  }
  let stream = Arc::new(
    PdfStream::open(pdf_path.to_str().expect("utf-8 path"))
      .expect("PdfStream should open valid test PDF"),
  );
  let (tx, rx) = mpsc::sync_channel(1);
  let mut editor = Editor::new(vec!["fast page".to_string()], 80);
  editor.ocr_enabled = true;
  editor.pdf_streaming = Some(PdfStreamingState {
    stream,
    col: 80,
    pages: vec![PageSlot::Loaded(LoadedPage::from_raw(
      "fast page".to_string(),
      80,
    ))],
    receiver: rx,
    cancel: Arc::new(AtomicBool::new(false)),
    fully_loaded: true,
    ocr_loading: true,
    ocr_receiver: None,
    ocr_cancel: None,
    ocr_worker: None,
    worker: None,
  });
  editor.rebuild_lines_from_pdf_stream();

  tx.send(PageLoaded::Page {
    page_index: 0,
    rendered_page: PdfRenderedPage {
      raw_text: "ocr page".to_string(),
      lines: vec!["ocr page".to_string()],
      line_kinds: vec![PdfLineKind::Text],
      contains_images: true,
    },
    replace_existing: true,
  })
  .expect("replacement page should enqueue");

  assert_eq!(editor.drain_pdf_stream(), 1);
  assert_eq!(editor.lines, vec!["ocr page"]);
  let PageSlot::Loaded(page) =
    &editor.pdf_streaming.as_ref().expect("streaming state").pages[0]
  else {
    panic!("page should be loaded");
  };
  assert!(page.ocr_enhanced);
}

#[test]
fn drain_pdf_stream_marks_ocr_complete() {
  let pdf_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../test-data/pdf/progit-1-50.pdf");
  if !pdf_path.exists() {
    return;
  }
  let stream = Arc::new(
    PdfStream::open(pdf_path.to_str().expect("utf-8 path"))
      .expect("PdfStream should open valid test PDF"),
  );
  let (tx, rx) = mpsc::sync_channel(1);
  let mut editor = Editor::new(vec!["fast page".to_string()], 80);
  editor.pdf_streaming = Some(PdfStreamingState {
    stream,
    col: 80,
    pages: vec![PageSlot::Loaded(LoadedPage::from_raw(
      "fast page".to_string(),
      80,
    ))],
    receiver: rx,
    cancel: Arc::new(AtomicBool::new(false)),
    fully_loaded: true,
    ocr_loading: true,
    ocr_receiver: None,
    ocr_cancel: None,
    ocr_worker: None,
    worker: None,
  });

  tx.send(PageLoaded::OcrComplete).expect("OCR completion should enqueue");

  assert_eq!(editor.drain_pdf_stream(), 1);
  assert!(!editor.pdf_streaming.as_ref().expect("streaming state").ocr_loading);
}

#[test]
fn ocr_replacement_preserves_page_line_and_cursor_screen_row() {
  let Some((mut editor, tx)) = editor_with_two_page_pdf() else {
    return;
  };
  editor.offset = 3;
  editor.cursor_y = 2;
  editor.buffers[0].offset = 3;
  editor.buffers[0].cursor_y = 2;
  assert_eq!(editor.current_pdf_position(), Some((2, 1)));

  tx.send(PageLoaded::Page {
    page_index: 0,
    rendered_page: rendered_image_page(&["p1 replacement"]),
    replace_existing: true,
  })
  .expect("replacement page should enqueue");

  assert_eq!(editor.drain_pdf_stream(), 1);
  assert_eq!(editor.current_pdf_position(), Some((2, 1)));
  assert_eq!(editor.cursor_y, 2);
  assert_eq!(editor.offset, 1);
}

#[test]
fn drain_pdf_stream_updates_pdf_buffer_without_moving_overlay() {
  assert_non_pdf_active_drain_preserves_active_buffer(ViewMode::Overlay);
}

#[test]
fn drain_pdf_stream_updates_pdf_buffer_without_moving_split() {
  assert_non_pdf_active_drain_preserves_active_buffer(
    ViewMode::HorizontalSplit,
  );
}

fn assert_non_pdf_active_drain_preserves_active_buffer(view_mode: ViewMode) {
  let Some((mut editor, tx)) = editor_with_two_page_pdf() else {
    return;
  };
  editor.buffers[0].offset = 3;
  editor.buffers[0].cursor_y = 2;

  let mut panel = BufferState::new(vec![
    "panel-0".to_string(),
    "panel-1".to_string(),
    "panel-2".to_string(),
    "panel-3".to_string(),
  ]);
  panel.offset = 1;
  panel.cursor_y = 1;
  panel.viewport_height = 4;
  panel.is_split_buffer = view_mode == ViewMode::HorizontalSplit;
  editor.buffers.push(panel);
  editor.active_buffer = 1;
  editor.active_pane = 1;
  editor.view_mode = view_mode.clone();
  editor.load_buffer_state(1);
  let active_lines = editor.lines.clone();
  let active_offset = editor.offset;
  let active_cursor_y = editor.cursor_y;
  let active_total_lines = editor.total_lines;

  tx.send(PageLoaded::Page {
    page_index: 0,
    rendered_page: rendered_image_page(&["p1 replacement"]),
    replace_existing: true,
  })
  .expect("replacement page should enqueue");

  assert_eq!(editor.drain_pdf_stream(), 1);
  assert_eq!(editor.active_buffer, 1);
  assert_eq!(editor.view_mode, view_mode);
  assert_eq!(editor.lines, active_lines);
  assert_eq!(editor.offset, active_offset);
  assert_eq!(editor.cursor_y, active_cursor_y);
  assert_eq!(editor.total_lines, active_total_lines);
  assert_eq!(editor.buffers[0].lines[0], "p1 replacement");
  assert_eq!(editor.buffers[0].offset, 1);
  assert_eq!(editor.buffers[0].cursor_y, 2);
}
