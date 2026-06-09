use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Receiver;
use std::thread::JoinHandle;

use cli_justify::PartialParagraph;
use cli_pdf_to_text::{PdfLineKind, PdfRenderedPage, SharedPdfStream};

/// Lines reserved for a Loading slot in the flat editor buffer.
pub const PLACEHOLDER_LINES_PER_PAGE: usize = 1;

#[derive(Clone)]
pub enum PageSlot {
  Loading,
  Loaded(LoadedPage),
}

#[derive(Clone)]
pub struct LoadedPage {
  /// Raw sanitized page text as returned by `PdfStream::extract_page`.
  /// Retained so the page can be re-justified on column-width changes
  /// without re-running PDF extraction.
  #[allow(dead_code)]
  pub raw_text: String,
  /// Standalone justified output for the page. Kept immutable so seam
  /// stitching can be recomputed idempotently as neighbours load.
  pub standalone_lines: Vec<String>,
  pub line_kinds: Vec<PdfLineKind>,
  pub contains_images: bool,
  pub ocr_enhanced: bool,
  /// Leading partial paragraph (if it looks like a continuation from the
  /// previous page).
  pub head_partial: Option<StoredPartial>,
  /// Trailing partial paragraph (if it looks incomplete and may continue
  /// onto the next page).
  pub tail_partial: Option<StoredPartial>,
}

#[derive(Clone)]
pub struct StoredPartial {
  pub raw_text: String,
  pub line_count: usize,
}

impl From<PartialParagraph> for StoredPartial {
  fn from(p: PartialParagraph) -> Self {
    Self { raw_text: p.raw_text, line_count: p.line_count }
  }
}

impl PageSlot {
  pub fn is_loaded(&self) -> bool {
    matches!(self, PageSlot::Loaded(_))
  }

  pub fn as_loaded(&self) -> Option<&LoadedPage> {
    if let PageSlot::Loaded(p) = self { Some(p) } else { None }
  }
}

/// Message posted by the background loader when page content or OCR state
/// changes. Justification happens on the main thread when the message is
/// drained.
pub enum PageLoaded {
  Page {
    page_index: usize,
    rendered_page: PdfRenderedPage,
    replace_existing: bool,
  },
  OcrComplete,
}

/// Result of the background "open the PDF + extract initial pages" job.
/// Sent once when the synchronous open + preload finish; afterwards the
/// loader thread continues with page-by-page extraction.
pub enum StreamReady {
  Ok {
    stream: SharedPdfStream,
    target_page: usize,
    restore_line_in_page: Option<usize>,
    /// Raw page text for every page extracted by the opener thread before
    /// the editor's first render. Includes the target page and a window
    /// of neighbours so the viewport is stable from the very first frame.
    /// Entries are `(page_1based, raw_text)`.
    preloaded_pages: Vec<(usize, PdfRenderedPage)>,
    pages_receiver: Receiver<PageLoaded>,
    cancel: Arc<AtomicBool>,
    worker: std::thread::JoinHandle<()>,
    ocr_loading: bool,
  },
  Err(String),
}

/// Held by the editor while the PDF is being opened in the background.
pub struct PendingPdfStream {
  pub receiver: std::sync::mpsc::Receiver<StreamReady>,
  /// When the open job was kicked off — surfaced as elapsed time in the
  /// loading splash so the user can see hygg hasn't frozen on a slow open.
  pub started_at: std::time::Instant,
  /// Display-friendly path so the splash can show *which* file is opening.
  pub canonical_path_display: String,
  /// Saved cursor row within the target page's rendered output, if any.
  /// Restored as cursor position once the preloaded pages are installed.
  pub restore_line_in_page: Option<usize>,
  /// Saved screen row for the cursor. Used during PDF restore so the
  /// first rendered frame lands on the same row as the previous session.
  pub restore_cursor_y: Option<usize>,
}

pub struct PdfStreamingState {
  /// Held so the underlying parsed document outlives the loader thread and
  /// stays available for on-demand re-extraction.
  #[allow(dead_code)]
  pub stream: SharedPdfStream,
  pub col: usize,
  pub pages: Vec<PageSlot>,
  pub receiver: Receiver<PageLoaded>,
  /// Signals the background loader to stop. Flipped on editor exit.
  pub cancel: Arc<AtomicBool>,
  /// True once every page has been received and stitched.
  pub fully_loaded: bool,
  pub ocr_loading: bool,
  pub ocr_receiver: Option<Receiver<PageLoaded>>,
  pub ocr_cancel: Option<Arc<AtomicBool>>,
  pub ocr_worker: Option<JoinHandle<()>>,
  /// Worker thread join handle; held so the thread is cleanly joined when
  /// the editor exits.
  pub worker: Option<JoinHandle<()>>,
}
