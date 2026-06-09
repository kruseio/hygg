use crate::core_types::ViewMode;
use crate::editor::core::Editor;
use crate::editor::streaming::{LoadedPage, PageSlot, PdfStreamingState};
use cli_pdf_to_text::PdfStream;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, mpsc};

#[test]
fn ocr_on_off_do_not_open_overlay_or_split_from_pdf_view() {
  let mut editor = Editor::new(vec!["pdf line".to_string()], 80);
  editor.active_buffer = 0;
  editor.view_mode = ViewMode::Normal;
  editor.buffers[0].command_buffer = "ocr on".to_string();

  editor.handle_ocr_command(true).expect("OCR on command should succeed");
  assert!(editor.ocr_enabled);
  assert_eq!(editor.active_buffer, 0);
  assert_eq!(editor.view_mode, ViewMode::Normal);
  assert_eq!(editor.buffers.len(), 1);

  editor.buffers[0].command_buffer = "ocr off".to_string();
  editor.handle_ocr_command(false).expect("OCR off command should succeed");
  assert!(!editor.ocr_enabled);
  assert_eq!(editor.active_buffer, 0);
  assert_eq!(editor.view_mode, ViewMode::Normal);
  assert_eq!(editor.buffers.len(), 1);
}

#[test]
fn ocr_on_starts_loader_and_stays_in_streaming_pdf_view() {
  let pdf_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../test-data/pdf/progit-1-50.pdf");
  if !pdf_path.exists() {
    return;
  }
  let stream = Arc::new(
    PdfStream::open(pdf_path.to_str().expect("utf-8 path"))
      .expect("PdfStream should open valid test PDF"),
  );
  let (_tx, rx) = mpsc::channel();
  let mut editor = Editor::new(vec!["pdf line".to_string()], 80);
  editor.pdf_source_path = Some(pdf_path.to_string_lossy().to_string());
  editor.pdf_streaming = Some(PdfStreamingState {
    stream,
    col: 80,
    pages: vec![PageSlot::Loaded(LoadedPage::from_raw(
      "pdf line".to_string(),
      80,
    ))],
    receiver: rx,
    cancel: Arc::new(AtomicBool::new(false)),
    fully_loaded: true,
    ocr_loading: false,
    ocr_receiver: None,
    ocr_cancel: None,
    ocr_worker: None,
    worker: None,
  });

  editor.handle_ocr_command(true).expect("OCR on command should succeed");

  assert!(editor.ocr_enabled);
  assert_eq!(editor.active_buffer, 0);
  assert_eq!(editor.view_mode, ViewMode::Normal);
  let state = editor.pdf_streaming.as_ref().expect("streaming state");
  assert!(state.ocr_loading);
  assert!(state.ocr_receiver.is_some());
  assert!(state.ocr_cancel.is_some());
  assert!(state.ocr_worker.is_some());

  let state = editor.pdf_streaming.as_mut().expect("streaming state");
  if let Some(cancel) = state.ocr_cancel.take() {
    cancel.store(true, std::sync::atomic::Ordering::Relaxed);
  }
  if let Some(worker) = state.ocr_worker.take() {
    let _ = worker.join();
  }
  state.ocr_receiver = None;
  state.ocr_loading = false;
}
