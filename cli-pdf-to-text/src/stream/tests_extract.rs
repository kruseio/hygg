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

/// Regression: progit page 43 (the "Skipping the Staging Area" page)
/// used to lose all paragraph breaks because pdf_oxide's text-line API
/// doesn't signal them — and the standalone "37" page-number footer
/// used to leak into content because the existing sanitize.rs heuristic
/// for footer numbers requires ≥20 chars of leading whitespace, which
/// our positional row builder strips. Verify both stay fixed.
#[test]
fn progit_paragraph_breaks_and_page_footer() {
  let pdf_path =
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../test-data/pdf/progit.pdf");
  if !pdf_path.exists() {
    return;
  }
  let stream = PdfStream::open(pdf_path.to_str().expect("utf-8 path"))
    .expect("PdfStream should open progit");
  let text =
    stream.extract_page(43).expect("progit page 43 should produce text");

  // Page-number footer must not leak through.
  let lines: Vec<&str> = text.lines().collect();
  assert!(
    !lines.iter().any(|l| l.trim() == "37"),
    "isolated page-number footer '37' should be stripped, got:\n{text}"
  );

  // The "Alternatively, you can type your commit message" sentence
  // starts a new paragraph after "and diff stripped out)." — there
  // should be a blank line between them so the reflowed output keeps
  // paragraph structure.
  let alt_pos = text
    .find("Alternatively, you can type your commit message")
    .expect("expected sentence on page 43");
  let before = &text[..alt_pos];
  assert!(
    before.trim_end().ends_with("and diff stripped out)."),
    "text immediately before 'Alternatively…' should end the previous \
     paragraph, got:\n…{}…",
    &before[before.len().saturating_sub(80)..]
  );
  let trailing_newlines =
    before.as_bytes().iter().rev().take_while(|&&b| b == b'\n').count();
  assert!(
    trailing_newlines >= 2,
    "expected at least one blank line before 'Alternatively…' \
     (a paragraph break), got {trailing_newlines} trailing newlines"
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
  let text = stream.extract_page(5).expect("page 5 should produce text");
  let lines: Vec<&str> = text.lines().collect();
  // Word-bbox-derived spacing now preserves the wide TOC gap between the
  // section title and its trailing page number, so the trimmed row keeps
  // multiple spaces between them. Match either spacing shape.
  let normalize_spaces =
    |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
  assert!(
    lines
      .iter()
      .any(|l| normalize_spaces(l.trim()) == "1.3 Related Publications 31"),
    "section 1.3 should be on its own line, got:\n{text}"
  );
  assert!(
    lines
      .iter()
      .any(|l| normalize_spaces(l.trim()) == "1.4 Intellectual Property 32"),
    "section 1.4 should be on its own line, got:\n{text}"
  );
  // The collapsing bug previously produced this run-on string.
  assert!(
    !text.contains("1.3 Related Publications1.4"),
    "section labels must not be concatenated, got:\n{text}"
  );
}

#[test]
fn sanitized_text_rows_keep_pdf_position_anchors() {
  let pdf_path = Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../test-data/pdf/progit-1-50.pdf");
  if !pdf_path.exists() {
    return;
  }
  let stream = PdfStream::open(pdf_path.to_str().expect("utf-8 path"))
    .expect("PdfStream should open valid test PDF");
  let raw_text = stream.extract_page(2).expect("page should produce text");
  let anchors = extract_visual_text_rows(&stream.doc, 1)
    .expect("page should produce positioned rows");
  let rows = positioned_sanitized_text_rows(&stream.doc, 1, &raw_text, 80);

  assert!(!rows.is_empty());
  assert_eq!(rows[0].top, anchors[0].top);
  assert_eq!(rows[0].left, anchors[0].left);
}
