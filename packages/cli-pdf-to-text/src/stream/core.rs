#[cfg(not(target_arch = "wasm32"))]
use hygg_shared::normalize_file_path;

use crate::sanitize::sanitize_layout_text;
use crate::stream::compose::compose_visual_page_with_overlay;
use crate::stream::images::render_pdf_images;
use crate::stream::text_lines::extract_page_text_lines;
use crate::stream::text_rows::{
  positioned_sanitized_text_rows, positioned_visual_text_rows,
  text_only_page_lines,
};
use crate::stream::types::{
  PdfPageForAnsi, PdfRenderedPage, PdfStream, VisualImageRows,
};
use crate::stream::vector::render_vector_diagram_regions;

#[cfg(feature = "ocr")]
use crate::stream::ocr::{
  has_near_duplicate_visual_text, ocr_visual_text_rows,
};

/// Upper bound on the page count taken from a PDF's catalog.
///
/// `page_count()` reads the document's declared `/Count`, which is a number the
/// file chooses — a few-kilobyte PDF can claim millions of pages. The
/// interactive reader turns that count into allocation directly: one `PageSlot`
/// per page, plus a page-load-order vector sized to match. pdf_oxide already
/// refuses a `/Count` beyond ~8.4M (it falls back to walking the page tree),
/// but 8.4M slots is still well over a gigabyte from nothing. No document a
/// person reads approaches this bound, so clamping it costs real files nothing
/// while keeping the up-front allocation bounded.
const MAX_STREAM_PAGES: usize = 200_000;

impl PdfStream {
  /// Open a PDF and parse its catalog. Does not extract any page text.
  ///
  /// Path-based; native-only. pdf_oxide gates its filesystem `open` off wasm,
  /// and there is no filesystem in the browser — the PWA uses [`open_bytes`].
  #[cfg(not(target_arch = "wasm32"))]
  pub fn open(pdf_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
    Self::open_with_optional_ocr(pdf_path, false)
  }

  #[cfg(not(target_arch = "wasm32"))]
  pub fn open_with_bundled_ocr(
    pdf_path: &str,
  ) -> Result<Self, Box<dyn std::error::Error>> {
    Self::open_with_optional_ocr(pdf_path, true)
  }

  /// Open a PDF from in-memory bytes — filesystem-free, for the browser PWA.
  ///
  /// pdf_oxide's `open_from_bytes` parses lazily just like the path-based
  /// `open`, so per-page extraction stays fast. `canonical_path` is left empty
  /// (there is no source file); OCR is never enabled on this path (the bundled
  /// engine is server-side), so scanned PDFs return empty text here.
  pub fn open_bytes(
    pdf_bytes: Vec<u8>,
  ) -> Result<Self, Box<dyn std::error::Error>> {
    let doc = pdf_oxide::PdfDocument::from_bytes(pdf_bytes)
      .map_err(|e| format!("pdf_oxide from_bytes failed: {e:?}"))?;
    let total_pages = doc
      .page_count()
      .map_err(|e| format!("pdf_oxide page_count failed: {e:?}"))?
      .min(MAX_STREAM_PAGES);
    Ok(Self {
      canonical_path: std::path::PathBuf::new(),
      doc,
      total_pages,
      #[cfg(feature = "ocr")]
      ocr_engine: None,
    })
  }

  /// Open an in-memory PDF with the bundled OCR engine attached — for the
  /// server-side `/convert` endpoint that OCRs scanned uploads. Native-only and
  /// gated on the `ocr` feature; pairs `from_bytes` with the OCR engine the
  /// path-based `open_with_bundled_ocr` uses.
  #[cfg(all(not(target_arch = "wasm32"), feature = "ocr"))]
  pub fn open_bytes_with_bundled_ocr(
    pdf_bytes: Vec<u8>,
  ) -> Result<Self, Box<dyn std::error::Error>> {
    let doc = pdf_oxide::PdfDocument::from_bytes(pdf_bytes)
      .map_err(|e| format!("pdf_oxide from_bytes failed: {e:?}"))?;
    let total_pages = doc
      .page_count()
      .map_err(|e| format!("pdf_oxide page_count failed: {e:?}"))?
      .min(MAX_STREAM_PAGES);
    Ok(Self {
      canonical_path: std::path::PathBuf::new(),
      doc,
      total_pages,
      ocr_engine: Some(crate::ocr::bundled_ocr_engine()?),
    })
  }

  #[cfg(not(target_arch = "wasm32"))]
  fn open_with_optional_ocr(
    pdf_path: &str,
    enable_ocr: bool,
  ) -> Result<Self, Box<dyn std::error::Error>> {
    let canonical_path = normalize_file_path(pdf_path)?;
    let doc = pdf_oxide::PdfDocument::open(&canonical_path)
      .map_err(|e| format!("pdf_oxide open failed: {e:?}"))?;
    let total_pages = doc
      .page_count()
      .map_err(|e| format!("pdf_oxide page_count failed: {e:?}"))?
      .min(MAX_STREAM_PAGES);
    #[cfg(feature = "ocr")]
    let ocr_engine =
      if enable_ocr { Some(crate::ocr::bundled_ocr_engine()?) } else { None };
    #[cfg(not(feature = "ocr"))]
    if enable_ocr {
      return Err(
        "OCR support is not available in this build. Rebuild with `--features ocr` to use the bundled English OCR engine."
          .into(),
      );
    }
    Ok(Self {
      canonical_path,
      doc,
      total_pages,
      #[cfg(feature = "ocr")]
      ocr_engine,
    })
  }

  pub fn total_pages(&self) -> usize {
    self.total_pages
  }

  pub fn canonical_path(&self) -> &std::path::Path {
    &self.canonical_path
  }

  /// Extract sanitized text for a single page.
  ///
  /// `page_index` is 1-based to match the historical lopdf-backed API
  /// (the rest of hygg counts pages from 1 in saved progress, status
  /// line, etc.). Returns `None` if the index is out of range, the page
  /// has no extractable text, or extraction panicked. pdf_oxide claims a
  /// 100 % pass rate on its 3 830-PDF corpus, but we still wrap in
  /// `catch_unwind` so a misbehaving page can't take down the background
  /// loader thread and leave every later page stuck on "loading".
  ///
  /// Uses pdf_oxide's positional `extract_text_lines` rather than the
  /// simpler `extract_text`. The former returns each visual line with
  /// its bounding box; we group lines that share a row (overlapping y
  /// ranges) and join them left-to-right. Without that step pdf_oxide
  /// can interleave adjacent TOC entries — "1.3 Foo1.4 Bar 3231" — and
  /// the downstream sanitizer can't recover them.
  pub fn extract_page(&self, page_index: usize) -> Option<String> {
    if page_index == 0 || page_index > self.total_pages {
      return None;
    }
    let doc = &self.doc;
    let page_0based = page_index - 1;
    let raw = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
      extract_page_text_lines(doc, page_0based)
    }))
    .ok()
    .flatten()?;
    if raw.trim().is_empty() {
      return None;
    }
    Some(sanitize_layout_text(&raw))
  }

  pub fn extract_page_with_images(
    &self,
    page_index: usize,
    col: usize,
  ) -> Option<PdfRenderedPage> {
    if page_index == 0 || page_index > self.total_pages {
      return None;
    }

    let raw_text = self.extract_page(page_index).unwrap_or_default();
    let page_0based = page_index - 1;
    let images = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
      self.doc.extract_images(page_0based)
    }))
    .ok()
    .and_then(Result::ok)
    .unwrap_or_default();

    let native_text_rows = positioned_visual_text_rows(&self.doc, page_0based);
    #[cfg(feature = "ocr")]
    let allow_unlabeled_vector_regions = self.ocr_engine.is_some();
    #[cfg(not(feature = "ocr"))]
    let allow_unlabeled_vector_regions = false;

    let mut image_rows =
      render_pdf_images(&self.doc, page_0based, col, images.as_slice());
    image_rows.extend(render_vector_diagram_regions(
      &self.doc,
      page_0based,
      col,
      &native_text_rows,
      allow_unlabeled_vector_regions,
    ));

    #[cfg(feature = "ocr")]
    let overlay_text_rows = {
      let mut text_rows = native_text_rows.clone();
      if let Some(engine) = self.ocr_engine.as_ref() {
        let ocr_rows = ocr_visual_text_rows(
          &self.doc,
          page_0based,
          images.as_slice(),
          engine,
          &text_rows,
        );
        let native_rows = text_rows.clone();
        text_rows.extend(
          ocr_rows
            .into_iter()
            .filter(|row| !has_near_duplicate_visual_text(&native_rows, row)),
        );
      }
      text_rows
    };
    #[cfg(not(feature = "ocr"))]
    let overlay_text_rows = native_text_rows.clone();

    if image_rows.is_empty() {
      let PdfPageForAnsi { lines, line_kinds } =
        text_only_page_lines(&raw_text, col);
      return Some(PdfRenderedPage {
        raw_text,
        lines,
        line_kinds,
        contains_images: false,
      });
    }

    let text_rows =
      positioned_sanitized_text_rows(&self.doc, page_0based, &raw_text, col);
    let PdfPageForAnsi { lines, line_kinds } = compose_visual_page_with_overlay(
      text_rows,
      overlay_text_rows,
      image_rows,
      col,
    );
    Some(PdfRenderedPage { raw_text, lines, line_kinds, contains_images: true })
  }
}
