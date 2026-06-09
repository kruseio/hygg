use super::*;
use cli_pdf_to_text::{PdfLineKind, PdfRenderedPage, PdfStream};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, mpsc};

fn rendered_image_page() -> PdfRenderedPage {
  PdfRenderedPage {
    raw_text: "caption text".to_string(),
    lines: vec![
      "caption text".to_string(),
      "\x1b[38;2;1;2;3m\x1b[48;2;4;5;6m▀\x1b[0m".to_string(),
    ],
    line_kinds: vec![PdfLineKind::Text, PdfLineKind::AnsiArt],
    contains_images: true,
  }
}

#[test]
fn rendered_image_pages_keep_fixed_lines_and_disable_partials() {
  let loaded = LoadedPage::from_rendered(rendered_image_page(), 80);

  assert!(loaded.contains_images);
  assert_eq!(loaded.standalone_lines.len(), 2);
  assert_eq!(loaded.line_kinds, vec![PdfLineKind::Text, PdfLineKind::AnsiArt]);
  assert!(loaded.head_partial.is_none());
  assert!(loaded.tail_partial.is_none());
}

#[test]
fn image_page_boundaries_use_separator_not_seam_stitching() {
  let before = LoadedPage::from_raw("This sentence continues".to_string(), 80);
  let image = LoadedPage::from_rendered(rendered_image_page(), 80);
  let after = LoadedPage::from_raw("afterward text".to_string(), 80);

  let before_count = before.rendered_line_count(None, Some(&image), false, 80);
  assert_eq!(before_count, before.standalone_lines.len() + 1);

  let image_count =
    image.rendered_line_count(Some(&before), Some(&after), false, 80);
  assert_eq!(image_count, image.standalone_lines.len() + 1);
}

#[test]
fn flat_line_kinds_track_art_rows() {
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
  let state = PdfStreamingState {
    stream,
    col: 80,
    pages: vec![
      PageSlot::Loaded(LoadedPage::from_raw("plain text".to_string(), 80)),
      PageSlot::Loaded(LoadedPage::from_rendered(rendered_image_page(), 80)),
    ],
    receiver: rx,
    cancel: Arc::new(AtomicBool::new(false)),
    fully_loaded: true,
    ocr_loading: false,
    ocr_receiver: None,
    ocr_cancel: None,
    ocr_worker: None,
    worker: None,
  };

  let lines = state.flat_lines();
  let kinds = state.flat_line_kinds();

  assert_eq!(lines.len(), kinds.len());
  assert!(kinds.contains(&PdfLineKind::AnsiArt));
  assert_eq!(state.page_line_count(0) + state.page_line_count(1), lines.len());
}
