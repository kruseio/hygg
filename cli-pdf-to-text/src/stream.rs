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
  /// `page_index` is 1-based. Returns `None` if the index is out of range
  /// or the page failed to render.
  pub fn extract_page(&self, page_index: usize) -> Option<String> {
    if page_index == 0 {
      return None;
    }
    let &page_num = self.page_numbers.get(page_index - 1)?;
    // See note on `open` above: no stdout silencing here either.
    let raw = render_page_layout_internal(&self.doc, page_num)?;
    Some(sanitize_layout_text(&raw))
  }
}

/// Convenience wrapper so callers can hold a cheap shared handle.
pub type SharedPdfStream = Arc<PdfStream>;

/// RAII guard that redirects stdout to /dev/null for its lifetime, mirroring
/// the dance `pdf_to_text` performs around extraction. Required because
/// `pdf-extract` / `lopdf` may emit warning messages to stdout while parsing
/// glyphs.
struct StdoutSilencer {
  #[cfg(not(target_os = "windows"))]
  state: Option<UnixStdoutState>,
}

#[cfg(not(target_os = "windows"))]
struct UnixStdoutState {
  saved_fd: i32,
  original_fd: i32,
}

impl StdoutSilencer {
  #[cfg(not(target_os = "windows"))]
  fn engage() -> Self {
    use std::fs::File;
    use std::os::fd::AsRawFd;

    let stdout = std::io::stdout();
    let original_fd = stdout.as_raw_fd();
    let saved_fd = unsafe { libc::dup(original_fd) };
    if saved_fd < 0 {
      return Self { state: None };
    }
    let Ok(dev_null) = File::open("/dev/null") else {
      unsafe {
        libc::close(saved_fd);
      }
      return Self { state: None };
    };
    unsafe {
      libc::dup2(dev_null.as_raw_fd(), original_fd);
    }
    Self { state: Some(UnixStdoutState { saved_fd, original_fd }) }
  }

  #[cfg(target_os = "windows")]
  fn engage() -> Self {
    let _ = redirect_stderr::redirect_stdout();
    Self {}
  }
}

impl Drop for StdoutSilencer {
  fn drop(&mut self) {
    #[cfg(not(target_os = "windows"))]
    if let Some(state) = self.state.take() {
      unsafe {
        libc::dup2(state.saved_fd, state.original_fd);
        libc::close(state.saved_fd);
      }
    }
    #[cfg(target_os = "windows")]
    {
      let _ = redirect_stderr::restore_stdout();
    }
  }
}

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

