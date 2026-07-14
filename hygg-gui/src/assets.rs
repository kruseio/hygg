//! Reader image assets: full-resolution figures and rasterized tables mapped
//! onto the open document's flattened lines, for the "Images" render mode.
//!
//! This is a pure render-layer overlay. It never touches the `Book`'s lines,
//! kinds, or page anchors — it only says "line range `L..L+N` can be drawn as
//! this raster instead of monospace." So an image asset covering a table's text
//! rows hides them visually while they still count toward the reading anchor,
//! keeping progress identical to every other client (which shows the text).
//!
//! Extraction is **viewport-driven and lazy**: the PDF is parsed once into a
//! long-lived [`AssetSource`], and the reader decodes only the pages
//! overlapping what's on screen (plus a small look-ahead) as you scroll.
//! Decoding the whole document up front cost seconds before the first figure
//! appeared on a large book; per-page decode is tens of milliseconds, so the
//! on-screen figure shows effectively at once. Extraction + placement live in
//! `cli_pdf_to_text` (shared with the PWA); this module runs them off-thread
//! and turns the result into iced handles.

use std::sync::Arc;

use cli_pdf_to_text::{PdfVisualExtractor, place_visuals};

use crate::model::LineKind;

/// A raster to draw over a contiguous run of document lines. `line_count` lines
/// (each one text row tall) are replaced by the image, so the document's total
/// height — and thus the scroll/anchor math — is unchanged.
#[derive(Clone, Debug)]
pub struct ImageAsset {
  pub line_start: usize,
  pub line_count: usize,
  pub handle: iced::widget::image::Handle,
}

/// A live, page-at-a-time visual source for the open PDF: the parsed document
/// plus the placement inputs (flattened lines, per-line image-ness, and page
/// anchors). Cheap to clone — every field is shared behind an `Arc` — so a copy
/// rides onto a worker thread for each page batch while the reader keeps one.
#[derive(Clone)]
pub struct AssetSource {
  extractor: Arc<PdfVisualExtractor>,
  lines: Arc<Vec<String>>,
  /// Per flattened line, whether it's an ASCII-art (image) row — the placement
  /// input, precomputed so the worker needn't know `LineKind`.
  is_img: Arc<Vec<bool>>,
  page_starts: Arc<Vec<usize>>,
  col: usize,
  /// 1-based page count, so the reader can clamp its look-ahead.
  pub total_pages: usize,
}

impl std::fmt::Debug for AssetSource {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("AssetSource")
      .field("total_pages", &self.total_pages)
      .field("col", &self.col)
      .finish_non_exhaustive()
  }
}

/// Open a page-at-a-time visual source for the book `id`. Reads the cached book
/// and source bytes and parses the PDF once (fast — pdf_oxide is lazy) on a
/// worker thread. `None` for non-PDFs, missing content, or a parse failure. The
/// reader keeps the source alive and calls [`extract_pages`] as it scrolls.
pub async fn open(id: String) -> Option<AssetSource> {
  let book = crate::storage::get_book(id.clone()).await?;
  if book.format != "pdf" {
    return None;
  }
  let bytes = crate::storage::get_blob(id).await?;
  let col = book.col.max(1);
  let (lines, kinds, page_starts) = (book.lines, book.kinds, book.page_starts);
  let (tx, rx) = iced::futures::channel::oneshot::channel();
  std::thread::spawn(move || {
    let src = PdfVisualExtractor::open(bytes).ok().map(|ex| {
      let is_img = kinds.iter().map(|k| matches!(k, LineKind::Ansi)).collect();
      AssetSource {
        total_pages: ex.total_pages(),
        extractor: Arc::new(ex),
        lines: Arc::new(lines),
        is_img: Arc::new(is_img),
        page_starts: Arc::new(page_starts),
        col,
      }
    });
    let _ = tx.send(src);
  });
  rx.await.ok().flatten()
}

/// Extract and place the given 1-based `pages`' visuals into drawable assets,
/// on a worker thread so the async runtime and UI stay responsive. Only these
/// pages are decoded — the reader requests the ones overlapping the viewport.
pub async fn extract_pages(
  src: AssetSource,
  pages: Vec<usize>,
) -> Vec<ImageAsset> {
  let (tx, rx) = iced::futures::channel::oneshot::channel();
  std::thread::spawn(move || {
    let _ = tx.send(src.build(&pages));
  });
  rx.await.unwrap_or_default()
}

impl AssetSource {
  /// Decode + place each page's visuals into iced handles.
  fn build(&self, pages: &[usize]) -> Vec<ImageAsset> {
    let is_img = |i: usize| self.is_img.get(i).copied().unwrap_or(false);
    let mut out = Vec::new();
    for &page in pages {
      let mut visuals = self.extractor.page(page, self.col);
      for p in place_visuals(&visuals, &self.lines, is_img, &self.page_starts) {
        let v = &mut visuals[p.visual];
        let handle = iced::widget::image::Handle::from_rgba(
          v.width,
          v.height,
          std::mem::take(&mut v.rgba),
        );
        out.push(ImageAsset {
          line_start: p.line_start,
          line_count: p.line_count,
          handle,
        });
      }
    }
    out
  }
}
