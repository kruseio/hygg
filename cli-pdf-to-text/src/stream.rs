use std::path::PathBuf;
use std::sync::Arc;

use hygg_shared::normalize_file_path;

use crate::render_page_layout_internal;
use crate::sanitize::sanitize_layout_text;

/// On-demand page extractor backed by a single parsed `pdf_extract::Document`.
///
/// `pdf_extract::Document` is `Sync`, so a `PdfStream` can be wrapped in
/// `Arc` and shared across the main thread (for the initially shown page)
/// and a background loader thread (for the rest of the document).
pub struct PdfStream {
  canonical_path: PathBuf,
  doc: pdf_extract::Document,
  page_numbers: Vec<u32>,
}

impl PdfStream {
  /// Open a PDF and parse its structure. Does not extract any page text.
  ///
  /// This is the fast path used by the streaming editor: it skips the
  /// up-front content-stream patching that `pdf_to_text` applies (which
  /// decompresses every page synchronously and dominates open time on
  /// large PDFs). Text emitted via the rare `'` / `"` text operators may
  /// be missing from individual pages as a result; users hitting that
  /// can fall back to the non-streaming pipeline via shell redirection
  /// (`hygg file.pdf | hygg` or similar).
  pub fn open(pdf_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
    let canonical_path = normalize_file_path(pdf_path)?;
    // No stdout-silencing here: `PdfStream::open` is intended to be called
    // from a background thread while the editor is interactively using the
    // terminal. fd 1 is process-global, so dup2'ing it would also silence
    // the editor's main thread and trip the "not a terminal" exit in the
    // main loop. We rely on `pdf_extract::Document::load` being quiet for
    // valid PDFs; any noise it does emit gets overwritten by the editor's
    // next redraw inside the alternate screen.
    let doc = pdf_extract::Document::load(&canonical_path)?;
    let mut page_numbers: Vec<u32> = doc.get_pages().into_keys().collect();
    page_numbers.sort_unstable();
    Ok(Self { canonical_path, doc, page_numbers })
  }

  pub fn total_pages(&self) -> usize {
    self.page_numbers.len()
  }

  pub fn canonical_path(&self) -> &std::path::Path {
    &self.canonical_path
  }

  /// Extract sanitized layout-aware text for a single page.
  ///
  /// `page_index` is 1-based. Returns `None` if the index is out of range,
  /// the page failed to render, or rendering panicked. `pdf-extract` / `lopdf`
  /// can panic on malformed content streams or unusual font encodings; we
  /// catch those so a single broken page does not take down the streaming
  /// loader thread and leave the editor stuck at "loading" for every other
  /// page in the document.
  pub fn extract_page(&self, page_index: usize) -> Option<String> {
    if page_index == 0 {
      return None;
    }
    let &page_num = self.page_numbers.get(page_index - 1)?;
    // See note on `open` above: no stdout silencing here either.
    let doc = &self.doc;
    let raw = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
      render_page_layout_internal(doc, page_num)
    }))
    .ok()
    .flatten()?;
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
