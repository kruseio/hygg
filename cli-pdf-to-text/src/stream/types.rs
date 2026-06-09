use std::path::PathBuf;
use std::sync::Arc;

/// On-demand page extractor backed by a single parsed `pdf_oxide::PdfDocument`.
///
/// pdf_oxide parses the file lazily — `open` does the xref + catalog and
/// returns in tens of milliseconds even on the 31 MB / 1310-page PDF
/// reference, where `lopdf::Document::load` (the old backend) took ~40 s
/// because it eagerly decompressed every content stream. Per-page
/// extraction is sub-millisecond warm, hundreds of micros cold.
///
/// `pdf_oxide::PdfDocument` is `Send + Sync` (its interior-mutable caches
/// are `Mutex`-guarded), so a `PdfStream` can be wrapped in `Arc` and
/// shared between the main thread (rendering the first visible page) and
/// the background loader thread (extracting the rest of the document) the
/// same way the lopdf-backed version was.
pub struct PdfStream {
  pub(crate) canonical_path: PathBuf,
  pub(crate) doc: pdf_oxide::PdfDocument,
  pub(crate) total_pages: usize,
  #[cfg(feature = "pdf-ocr-bundled")]
  pub(crate) ocr_engine: Option<pdf_oxide::ocr::OcrEngine>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PdfLineKind {
  Text,
  AnsiArt,
}

#[derive(Clone, Debug)]
pub struct PdfRenderedPage {
  pub raw_text: String,
  pub lines: Vec<String>,
  pub line_kinds: Vec<PdfLineKind>,
  pub contains_images: bool,
}

pub(crate) struct PdfPageForAnsi {
  pub(crate) lines: Vec<String>,
  pub(crate) line_kinds: Vec<PdfLineKind>,
}

#[derive(Clone, Debug)]
pub(crate) struct VisualTextRow {
  pub(crate) top: f32,
  pub(crate) left: f32,
  pub(crate) text: String,
}

pub(crate) struct VisualImageRows {
  pub(crate) top: f32,
  pub(crate) left_cells: usize,
  pub(crate) width_cells: usize,
  pub(crate) region: PdfRegion,
  pub(crate) lines: Vec<String>,
}

pub(crate) const PDF_TEXT_PT_PER_CHAR: f32 = 5.0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct PdfRegion {
  pub(crate) left: f32,
  pub(crate) bottom: f32,
  pub(crate) width: f32,
  pub(crate) height: f32,
}

impl PdfRegion {
  pub(crate) fn top(&self) -> f32 {
    self.bottom + self.height
  }
}

/// Convenience wrapper so callers can hold a cheap shared handle.
pub type SharedPdfStream = Arc<PdfStream>;
