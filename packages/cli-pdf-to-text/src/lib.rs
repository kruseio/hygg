// The non-streaming, layout-fidelity `pdf_to_text` path (lopdf + pdf-extract +
// rayon + redirect-stderr) is native-only — those deps don't build for wasm32.
// The browser PWA reuses the pure pdf_oxide streaming path via the byte-input
// `pdf_bytes_to_ansi_text` / `PdfStream::open_bytes`, which is portable.

#[cfg(not(target_arch = "wasm32"))]
use hygg_shared::normalize_file_path;
#[cfg(not(target_arch = "wasm32"))]
use rayon::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use std::io::{BufWriter, Cursor};

// Common to both targets: the wasm `sanitize` path uses `is_code_like_line`;
// the rest of the module is gated native-only inside `heuristics.rs`.
mod heuristics;
#[cfg(not(target_arch = "wasm32"))]
mod layout_text_output;
#[cfg(not(target_arch = "wasm32"))]
mod ocr;
mod paged;
#[cfg(not(target_arch = "wasm32"))]
mod pdf_patch;
mod sanitize;
mod stream;
#[cfg(not(target_arch = "wasm32"))]
mod stream_recovery;
#[cfg(feature = "visual-assets")]
mod visual_place;
#[cfg(feature = "visual-assets")]
mod visuals;

pub use stream::{PdfLineKind, PdfRenderedPage, PdfStream, SharedPdfStream};
#[cfg(feature = "visual-assets")]
pub use visual_place::{VisualPlacement, place_visuals};
#[cfg(feature = "visual-assets")]
pub use visuals::{
  PdfVisual, PdfVisualExtractor, PdfVisualKind, pdf_bytes_to_visuals,
};

#[cfg(not(target_arch = "wasm32"))]
use heuristics::{
  layout_needs_plaintext_fallback, should_prefer_plaintext_output,
};
#[cfg(not(target_arch = "wasm32"))]
use sanitize::sanitize_layout_text;
#[cfg(not(target_arch = "wasm32"))]
use stream_recovery::recover_sparse_code_blocks;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn load_patched_doc_internal(
  canonical_path: &std::path::Path,
) -> Result<pdf_extract::Document, Box<dyn std::error::Error>> {
  match pdf_patch::patched_pdf_bytes(canonical_path) {
    Ok(bytes) => match pdf_extract::Document::load_mem(&bytes) {
      Ok(doc) => Ok(doc),
      Err(_) => Ok(pdf_extract::Document::load(canonical_path)?),
    },
    Err(_) => Ok(pdf_extract::Document::load(canonical_path)?),
  }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn render_page_layout_internal(
  doc: &pdf_extract::Document,
  page_num: u32,
) -> Option<String> {
  let mut buf = Vec::new();
  {
    let mut writer = BufWriter::new(Cursor::new(&mut buf));
    let mut output = layout_text_output::LayoutTextOutput::new(
      &mut writer as &mut dyn std::io::Write,
    );
    pdf_extract::output_doc_page(doc, &mut output, page_num).ok()?;
  }
  String::from_utf8(buf).ok()
}

/// Extract layout-aware text from every page in parallel.
///
/// `pdf_extract::Document` (a re-export of `lopdf::Document`) is
/// `Send + Sync`, so we share one parsed instance across rayon
/// workers via reference. Per-page output is collected and
/// concatenated in page order.
#[cfg(not(target_arch = "wasm32"))]
fn extract_with_layout_text(
  canonical_path: &std::path::Path,
) -> Result<String, Box<dyn std::error::Error>> {
  let doc = load_patched_doc_internal(canonical_path)?;
  pdf_extract::print_metadata(&doc);

  let mut page_nums: Vec<u32> = doc.get_pages().into_keys().collect();
  page_nums.sort_unstable();

  // par_iter().collect() preserves source order, so the resulting Vec
  // is already in page order without an extra sort.
  let pages: Vec<Option<String>> = page_nums
    .par_iter()
    .map(|&page_num| render_page_layout_internal(&doc, page_num))
    .collect();

  let mut combined = String::new();
  for page in pages.into_iter().flatten() {
    combined.push_str(&page);
  }
  Ok(combined)
}

/// The passes that must run with stdout redirected, in one fallible unit.
///
/// Returns the sanitized layout text and, when the layout pass came out damaged
/// enough to be worth a second opinion, the plaintext fallback's raw output.
#[cfg(not(target_arch = "wasm32"))]
fn extract_under_redirected_stdout(
  canonical_path: &std::path::Path,
) -> Result<(String, Option<String>), Box<dyn std::error::Error>> {
  let layout_text = extract_with_layout_text(canonical_path)?;
  let mut layout_sanitized = sanitize_layout_text(&layout_text);

  if let Ok(Some(recovered)) =
    recover_sparse_code_blocks(canonical_path, &layout_sanitized)
  {
    layout_sanitized = recovered;
  }

  // Only run the slower plaintext fallback when the layout pass shows
  // damage that the plaintext heuristic might actually prefer. On large
  // PDFs this halves wall time.
  let plaintext_result = if layout_needs_plaintext_fallback(&layout_sanitized) {
    pdf_extract::extract_text(canonical_path).ok()
  } else {
    None
  };

  Ok((layout_sanitized, plaintext_result))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn pdf_to_text(
  pdf_path: &str,
) -> Result<String, Box<dyn std::error::Error>> {
  let canonical_path = normalize_file_path(pdf_path)?;

  // `redirect_stderr::redirect_stdout` works on both Windows and Unix now;
  // suppress the noisy logging pdf_extract / lopdf write to stdout while we
  // do the extraction passes.
  //
  // The extraction is a separate function so that the restore below is not
  // something the code between here and there can jump over. It used to be one
  // straight-line body with a `?` in the middle, and that `?` returned with the
  // process's stdout still pointing at /dev/null — a malformed PDF thus
  // silently muted every later `println!` in the program. This is a library;
  // the caller is not always about to exit.
  redirect_stderr::redirect_stdout()?;
  let extracted = extract_under_redirected_stdout(&canonical_path);
  redirect_stderr::restore_stdout()?;
  let (layout_sanitized, plaintext_result) = extracted?;

  if let Some(plaintext_output) = plaintext_result {
    let plaintext_sanitized = sanitize_layout_text(&plaintext_output);
    if should_prefer_plaintext_output(&layout_sanitized, &plaintext_sanitized) {
      return Ok(plaintext_sanitized);
    }
  }

  Ok(layout_sanitized)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn pdf_to_text_with_bundled_ocr(
  pdf_path: &str,
) -> Result<String, Box<dyn std::error::Error>> {
  ocr::pdf_to_text_with_bundled_ocr(pdf_path)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn pdf_to_ansi_text(
  pdf_path: &str,
  col: usize,
) -> Result<String, Box<dyn std::error::Error>> {
  let stream = PdfStream::open(pdf_path)?;
  pdf_stream_to_ansi_text(&stream, col)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn pdf_to_ansi_text_with_bundled_ocr(
  pdf_path: &str,
  col: usize,
) -> Result<String, Box<dyn std::error::Error>> {
  let stream = PdfStream::open_with_bundled_ocr(pdf_path)?;
  pdf_stream_to_ansi_text(&stream, col)
}

/// Extract a whole in-memory PDF to ANSI/justified text (`col`-wide), reusing
/// the streaming pdf_oxide backend. Filesystem-free, so it works in the browser
/// PWA: pass the raw bytes of an imported `.pdf` `File`.
pub fn pdf_bytes_to_ansi_text(
  pdf_bytes: Vec<u8>,
  col: usize,
) -> Result<String, Box<dyn std::error::Error>> {
  let stream = PdfStream::open_bytes(pdf_bytes)?;
  pdf_stream_to_ansi_text(&stream, col)
}

/// Like [`pdf_bytes_to_ansi_text`] but runs the bundled OCR engine over image
/// regions — for the server `/convert` endpoint extracting scanned PDFs from
/// in-memory bytes. Native-only, gated on the `ocr` feature.
#[cfg(all(not(target_arch = "wasm32"), feature = "ocr"))]
pub fn pdf_bytes_to_ansi_text_with_bundled_ocr(
  pdf_bytes: Vec<u8>,
  col: usize,
) -> Result<String, Box<dyn std::error::Error>> {
  let stream = PdfStream::open_bytes_with_bundled_ocr(pdf_bytes)?;
  pdf_stream_to_ansi_text(&stream, col)
}

/// Extract a whole in-memory PDF to per-line `(text, kind)` pairs (`col`-wide).
///
/// Like [`pdf_bytes_to_ansi_text`] but preserves each line's [`PdfLineKind`] so
/// a DOM/canvas frontend can render ASCII-art rows differently from prose
/// (the flattened string form loses that distinction). Pages are separated by a
/// single blank `Text` line, matching the reader's inter-page spacing.
pub fn pdf_bytes_to_lines(
  pdf_bytes: Vec<u8>,
  col: usize,
) -> Result<Vec<(String, PdfLineKind)>, Box<dyn std::error::Error>> {
  Ok(pdf_bytes_to_lines_paged(pdf_bytes, col)?.0)
}

/// Like [`pdf_bytes_to_lines`] but also returns, for each 1-based PDF page, the
/// index of the first output line belonging to it (`page_starts[0]` is always
/// 0). A frontend that stores the flattened lines can use this to recover a
/// stable, pagination-independent `(page, line_in_page)` position — the anchor
/// cross-device sync restores by, so two readers that wrap the document at
/// different widths still resume on the same page.
///
/// The assembly applies the same cross-page seam stitching and 0-or-1
/// inter-page spacing as the terminal reader's streaming `flat_lines`, so the
/// flat buffer (and thus every page-local resume anchor) is byte-identical
/// across clients.
#[allow(clippy::type_complexity)]
pub fn pdf_bytes_to_lines_paged(
  pdf_bytes: Vec<u8>,
  col: usize,
) -> Result<(Vec<(String, PdfLineKind)>, Vec<usize>), Box<dyn std::error::Error>>
{
  let stream = PdfStream::open_bytes(pdf_bytes)?;
  Ok(paged::assemble_paged(&stream, col))
}

fn pdf_stream_to_ansi_text(
  stream: &PdfStream,
  col: usize,
) -> Result<String, Box<dyn std::error::Error>> {
  let mut output = Vec::new();
  for page in 1..=stream.total_pages() {
    let Some(rendered) = stream.extract_page_with_images(page, col) else {
      continue;
    };
    output.extend(rendered.lines);
    if page < stream.total_pages() {
      output.push(String::new());
    }
  }
  Ok(output.join("\n"))
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
