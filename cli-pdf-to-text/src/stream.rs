use std::path::PathBuf;
use std::sync::Arc;

use hygg_shared::normalize_file_path;

use crate::sanitize::sanitize_layout_text;

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
  canonical_path: PathBuf,
  doc: pdf_oxide::PdfDocument,
  total_pages: usize,
}

impl PdfStream {
  /// Open a PDF and parse its catalog. Does not extract any page text.
  pub fn open(pdf_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
    let canonical_path = normalize_file_path(pdf_path)?;
    let doc = pdf_oxide::PdfDocument::open(&canonical_path)
      .map_err(|e| format!("pdf_oxide open failed: {e:?}"))?;
    let total_pages = doc
      .page_count()
      .map_err(|e| format!("pdf_oxide page_count failed: {e:?}"))?;
    Ok(Self { canonical_path, doc, total_pages })
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
  pub fn extract_page(&self, page_index: usize) -> Option<String> {
    if page_index == 0 || page_index > self.total_pages {
      return None;
    }
    let doc = &self.doc;
    let page_0based = page_index - 1;
    let raw = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
      doc.extract_text(page_0based).ok()
    }))
    .ok()
    .flatten()?;
    if raw.trim().is_empty() {
      return None;
    }
    Some(sanitize_layout_text(&raw))
  }
}

/// Convenience wrapper so callers can hold a cheap shared handle.
pub type SharedPdfStream = Arc<PdfStream>;

#[cfg(test)]
mod tests {
  use super::*;
  use std::path::Path;

  #[test]
  fn opens_and_extracts_individual_pages() {
    let pdf_path = Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("../test-data/pdf/progit-1-50.pdf");
    if !pdf_path.exists() {
      return;
    }
    let stream = PdfStream::open(pdf_path.to_str().expect("utf-8 path"))
      .expect("PdfStream should open valid test PDF");
    assert!(stream.total_pages() > 0, "test PDF should report pages");

    // Scan a few early pages — at least one should produce real text.
    // (The first page of progit is a title/cover with minimal text.)
    let scan_upto = stream.total_pages().min(5);
    let mut any_non_empty = false;
    for p in 1..=scan_upto {
      if let Some(text) = stream.extract_page(p)
        && !text.trim().is_empty()
      {
        any_non_empty = true;
        break;
      }
    }
    assert!(
      any_non_empty,
      "at least one of the first {scan_upto} pages should extract non-empty text"
    );
  }
}
