use hygg_shared::normalize_file_path;
use rayon::prelude::*;
use std::io::{BufWriter, Cursor};

mod heuristics;
mod layout_text_output;
mod pdf_patch;
mod sanitize;
mod stream;
mod stream_recovery;

pub use stream::{PdfStream, SharedPdfStream};

use heuristics::{
  layout_needs_plaintext_fallback, should_prefer_plaintext_output,
};
use sanitize::sanitize_layout_text;
use stream_recovery::recover_sparse_code_blocks;

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

pub fn pdf_to_text(
  pdf_path: &str,
) -> Result<String, Box<dyn std::error::Error>> {
  let canonical_path = normalize_file_path(pdf_path)?;

  // `redirect_stderr::redirect_stdout` works on both Windows and Unix now;
  // suppress the noisy logging pdf_extract / lopdf write to stdout while we
  // do the extraction passes.
  redirect_stderr::redirect_stdout()?;

  let layout_text = extract_with_layout_text(&canonical_path);

  let layout_text = layout_text?;
  let mut layout_sanitized = sanitize_layout_text(&layout_text);

  if let Ok(Some(recovered)) =
    recover_sparse_code_blocks(&canonical_path, &layout_sanitized)
  {
    layout_sanitized = recovered;
  }

  // Only run the slower plaintext fallback when the layout pass shows
  // damage that the plaintext heuristic might actually prefer. On large
  // PDFs this halves wall time.
  let plaintext_result = if layout_needs_plaintext_fallback(&layout_sanitized) {
    pdf_extract::extract_text(&canonical_path).ok()
  } else {
    None
  };

  redirect_stderr::restore_stdout()?;

  if let Some(plaintext_output) = plaintext_result {
    let plaintext_sanitized = sanitize_layout_text(&plaintext_output);
    if should_prefer_plaintext_output(&layout_sanitized, &plaintext_sanitized) {
      return Ok(plaintext_sanitized);
    }
  }

  Ok(layout_sanitized)
}

#[cfg(test)]
mod tests {
  use std::path::Path;

  use super::{pdf_to_text, should_prefer_plaintext_output};

  #[test]
  fn keeps_layout_when_plaintext_has_no_structural_gain() {
    let layout = concat!(
      "A Heading\n",
      "Some explanatory text.\n",
      "Another paragraph.\n",
    );
    let plaintext = concat!(
      "A Heading\n",
      "Some explanatory text.\n",
      "Another paragraph.\n",
      "Noise line\n",
    );
    assert!(!should_prefer_plaintext_output(layout, plaintext));
  }

  #[test]
  fn keeps_progit_codeblock_lines_in_output() {
    let pdf_path = Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("../test-data/pdf/progit-1-50.pdf");
    if !pdf_path.exists() {
      return;
    }

    let text = pdf_to_text(
      pdf_path.to_str().expect("test PDF path should be valid UTF-8"),
    )
    .expect("expected pdf_to_text to succeed for progit sample");

    for expected in
      ["*.a", "!lib.a", "/TODO", "build/", "doc/*.txt", "doc/**/*.pdf"]
    {
      assert!(
        text.contains(expected),
        "expected recovered codeblock to contain {expected:?}, got excerpt around heading: {:?}",
        text
          .lines()
          .skip_while(|line| {
            !line.contains("Here is another example .gitignore file:")
          })
          .take(40)
          .collect::<Vec<_>>()
      );
    }
  }
}
