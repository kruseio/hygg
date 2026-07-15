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

/// The batch PWA/GUI assembly (`cli_pdf_to_text::pdf_bytes_to_lines_paged`)
/// must produce a byte-identical flat buffer — lines, kinds, and per-page start
/// indices — to the terminal reader's streaming `flat_lines` once every page is
/// loaded. This is what makes a page-local resume anchor land on the same
/// content in every client; if the two assemblies ever drift (different seam
/// stitching or inter-page spacing), cross-client PDF resume silently skews and
/// this test fails.
#[test]
fn batch_paged_assembly_matches_streaming_flat_buffer() {
  let pdf_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../test-data/pdf/progit-1-50.pdf");
  if !pdf_path.exists() {
    return;
  }
  let col = 80usize;
  let bytes = std::fs::read(&pdf_path).expect("read test PDF");
  let stream = Arc::new(
    PdfStream::open(pdf_path.to_str().expect("utf-8 path"))
      .expect("open test PDF"),
  );
  let total = stream.total_pages();
  assert!(total > 20);

  // Load every page exactly as the streaming loader does (extract with images,
  // then `from_rendered`), giving a fully-loaded state.
  let pages: Vec<PageSlot> = (1..=total)
    .map(|p| {
      let rendered = stream
        .extract_page_with_images(p, col)
        .expect("extract page with images");
      PageSlot::Loaded(LoadedPage::from_rendered(rendered, col))
    })
    .collect();
  let (_tx, rx) = mpsc::channel();
  let state = PdfStreamingState {
    stream: Arc::clone(&stream),
    col,
    pages,
    receiver: rx,
    cancel: Arc::new(AtomicBool::new(false)),
    fully_loaded: true,
    ocr_loading: false,
    ocr_receiver: None,
    ocr_cancel: None,
    ocr_worker: None,
    worker: None,
  };

  let cli_lines = state.flat_lines();
  let cli_kinds = state.flat_line_kinds();
  let cli_starts: Vec<usize> =
    (0..total).map(|i| state.line_start_for_page(i)).collect();

  let (paged, paged_starts) =
    cli_pdf_to_text::pdf_bytes_to_lines_paged(bytes, col).expect("batch paged");
  let paged_lines: Vec<String> =
    paged.iter().map(|(line, _)| line.clone()).collect();
  let paged_kinds: Vec<PdfLineKind> =
    paged.iter().map(|(_, kind)| *kind).collect();

  assert_eq!(cli_lines, paged_lines, "flat lines diverged");
  assert_eq!(cli_kinds, paged_kinds, "flat line kinds diverged");
  assert_eq!(cli_starts, paged_starts, "page start indices diverged");
}

#[test]
fn flat_line_kinds_track_art_rows() {
  let pdf_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../test-data/pdf/progit-1-50.pdf");
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
