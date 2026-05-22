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
}

/// Build a text blob from pdf_oxide's positional `TextLine` output.
///
/// Lines are returned in a roughly visual order but adjacent rows can
/// collide when text is laid out in cells (table rows) or columns. We
/// sort by y descending (PDF origin is bottom-left, so top of page is the
/// largest y), then walk the list collecting lines that share a row into
/// a single output line, sorted left-to-right within the row.
fn extract_page_text_lines(
  doc: &pdf_oxide::PdfDocument,
  page_0based: usize,
) -> Option<String> {
  let mut lines = doc.extract_text_lines(page_0based).ok()?;
  if lines.is_empty() {
    return None;
  }

  // Sort top-to-bottom, then left-to-right.
  lines.sort_by(|a, b| {
    b.bbox
      .top()
      .partial_cmp(&a.bbox.top())
      .unwrap_or(std::cmp::Ordering::Equal)
      .then_with(|| {
        a.bbox
          .left()
          .partial_cmp(&b.bbox.left())
          .unwrap_or(std::cmp::Ordering::Equal)
      })
  });

  // Threshold below which two lines are considered to be on the same row.
  // pdf_oxide's line bboxes for the same baseline tend to differ by < 1pt
  // even with mixed font sizes; 3pt comfortably absorbs that noise without
  // merging adjacent rows (which are typically separated by 10+pt).
  const SAME_ROW_TOL: f32 = 3.0;

  let mut output = String::new();
  let mut row_start = 0usize;
  let mut row_anchor_y = lines[0].bbox.top();

  for i in 1..=lines.len() {
    let break_row = i == lines.len()
      || (row_anchor_y - lines[i].bbox.top()).abs() > SAME_ROW_TOL;
    if break_row {
      // Flush rows[row_start..i] sorted by x ascending (already roughly so
      // because of the secondary sort, but re-sort defensively in case
      // floating-point ties were broken the other way).
      let mut row: Vec<&pdf_oxide::layout::TextLine> =
        lines[row_start..i].iter().collect();
      row.sort_by(|a, b| {
        a.bbox
          .left()
          .partial_cmp(&b.bbox.left())
          .unwrap_or(std::cmp::Ordering::Equal)
      });
      let joined: Vec<&str> =
        row.iter().map(|l| l.text.as_str()).collect();
      output.push_str(&joined.join(" "));
      output.push('\n');
      row_start = i;
      if i < lines.len() {
        row_anchor_y = lines[i].bbox.top();
      }
    }
  }

  Some(output)
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

  /// Regression: the pdf reference 1.7 TOC interleaves two adjacent
  /// section headers because `extract_text` collapses lines without
  /// regard to their bounding boxes. `extract_text_lines` + the
  /// row-grouping in `extract_page_text_lines` is what fixes it, so make
  /// sure section labels stay on their own lines for a TOC-shaped page.
  #[test]
  fn toc_section_labels_stay_separate() {
    let pdf_path = Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("../test-data/pdf/pdfreference1.7old.pdf");
    if !pdf_path.exists() {
      return;
    }
    let stream = PdfStream::open(pdf_path.to_str().expect("utf-8 path"))
      .expect("PdfStream should open the reference PDF");
    // Page 5 (1-based) is the contents page.
    let text = stream
      .extract_page(5)
      .expect("page 5 should produce text");
    let lines: Vec<&str> = text.lines().collect();
    assert!(
      lines.iter().any(|l| l.trim() == "1.3 Related Publications 31"),
      "section 1.3 should be on its own line, got:\n{text}"
    );
    assert!(
      lines.iter().any(|l| l.trim() == "1.4 Intellectual Property 32"),
      "section 1.4 should be on its own line, got:\n{text}"
    );
    // The collapsing bug previously produced this run-on string.
    assert!(
      !text.contains("1.3 Related Publications1.4"),
      "section labels must not be concatenated, got:\n{text}"
    );
  }
}
