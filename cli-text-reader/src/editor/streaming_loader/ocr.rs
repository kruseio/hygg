use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread::{self, JoinHandle};

use cli_pdf_to_text::PdfStream;

use super::super::streaming::PageLoaded;
use super::loader::{
  CHANNEL_BUFFER, load_order, render_or_blank, send_page_loaded,
};

pub fn spawn_ocr_loader(
  pdf_path: String,
  start_page: usize,
  col: usize,
  total_pages: usize,
  cancel: Arc<AtomicBool>,
) -> (Receiver<PageLoaded>, JoinHandle<()>) {
  let (tx, rx) = mpsc::sync_channel::<PageLoaded>(CHANNEL_BUFFER);
  let handle = thread::Builder::new()
    .name("hygg-pdf-ocr-loader".into())
    .spawn(move || {
      if let Ok(stream) = PdfStream::open_with_bundled_ocr(&pdf_path) {
        run_ocr_page(&stream, start_page, col, &tx, &cancel);
        run_ocr_remaining_pages(
          &stream,
          start_page,
          col,
          total_pages,
          &tx,
          &cancel,
        );
      }
      send_page_loaded(&tx, PageLoaded::OcrComplete, cancel.as_ref());
    })
    .expect("spawning pdf OCR loader thread");

  (rx, handle)
}

fn run_ocr_page(
  stream: &PdfStream,
  page_1based: usize,
  col: usize,
  tx: &SyncSender<PageLoaded>,
  cancel: &AtomicBool,
) {
  if cancel.load(Ordering::Relaxed) {
    return;
  }
  if page_1based == 0 || page_1based > stream.total_pages() {
    return;
  }
  let rendered_page = render_or_blank(stream, page_1based, col);
  let message = PageLoaded::Page {
    page_index: page_1based - 1,
    rendered_page,
    replace_existing: true,
  };
  send_page_loaded(tx, message, cancel);
}

fn run_ocr_remaining_pages(
  stream: &PdfStream,
  start_page: usize,
  col: usize,
  total_pages: usize,
  tx: &SyncSender<PageLoaded>,
  cancel: &AtomicBool,
) {
  for page_1based in load_order(start_page, total_pages) {
    run_ocr_page(stream, page_1based, col, tx, cancel);
    if cancel.load(Ordering::Relaxed) {
      break;
    }
  }
}
