use crate::sanitize::chars::normalize_extracted_line;
use crate::sanitize::headers::is_running_header_or_footer_line;
use crate::sanitize::labels::centered_heading_label;
use crate::sanitize::sanitize_layout_text;

#[test]
fn removes_vertical_margin_letter_lines_and_excess_blank_lines() {
  let input = concat!(
    "Contents 8\n",
    "                                                                                                   C\n",
    "                                                                                                   o\n",
    "                                                                                                   n\n",
    "\n",
    "\n",
    "\n",
    "\n",
    "Body\n"
  );

  let output = sanitize_layout_text(input);
  assert!(!output.contains("\n                                                                                                   C\n"));
  assert!(!output.contains("\n                                                                                                   o\n"));
  assert!(!output.contains("\n                                                                                                   n\n"));
  assert!(!output.contains("\n\n\n\n\n"));
  assert!(output.contains("Contents 8"));
  assert!(output.contains("Body"));
}

#[test]
fn keeps_normal_single_letter_lines() {
  let input = "A\n  B\nShort line\n";
  let output = sanitize_layout_text(input);

  assert!(output.contains("\nA\n") || output.starts_with("A\n"));
  assert!(output.contains("\n  B\n") || output.starts_with("  B\n"));
  assert!(output.contains("Short line\n"));
}

#[test]
fn removes_running_header_and_footer_lines() {
  let input = concat!(
    "                                                                                                           IntroductionCHAPTER 1                                         28\n",
    "  Preface                                                 24\n",
    "Body paragraph line\n"
  );

  let output = sanitize_layout_text(input);
  assert!(!output.contains("IntroductionCHAPTER 1"));
  assert!(
    !output
      .contains("Preface                                                 24")
  );
  assert!(output.contains("Body paragraph line"));
}

#[test]
fn drops_per_page_chapter_section_running_headers() {
  // pdf_oxide preserves these at the top of every page within a
  // chapter / section. Without filtering, "SECTION 3.2 Objects"
  // shows up wedged between the previous page's last line and the
  // current page's first line, breaking a paragraph that crossed
  // the page boundary.
  let chapter =
    "CHAPTER 3                                                    Syntax";
  let section =
    "SECTION 3.2                                                   Objects";
  let appendix =
    "APPENDIX A                                                   Notes";
  assert!(
    is_running_header_or_footer_line(chapter),
    "expected chapter running header to be dropped"
  );
  assert!(
    is_running_header_or_footer_line(section),
    "expected section running header to be dropped"
  );
  assert!(
    is_running_header_or_footer_line(appendix),
    "expected appendix running header to be dropped"
  );
}

#[test]
fn drops_left_aligned_front_matter_running_heads() {
  // "Figures" at column 0 on every page after the actual centered
  // heading is a running head that must be filtered. The centered
  // version on the first page survives via `centered_heading_label`.
  assert!(is_running_header_or_footer_line("Figures"));
  assert!(is_running_header_or_footer_line("Tables"));
  assert!(is_running_header_or_footer_line("Contents"));
}

#[test]
fn keeps_centered_section_heading() {
  // The actual section heading is centered (≥12 leading spaces) and
  // must survive — only the left-aligned variant is a running head.
  assert!(!is_running_header_or_footer_line("                    Figures"));
}

#[test]
fn keeps_real_chapter_title_lines() {
  // The actual chapter title page in PDF Reference uses "3 Syntax"
  // (number + title, no CHAPTER prefix). It must survive the filter.
  let chapter_title = "                    3 Syntax";
  assert!(
    !is_running_header_or_footer_line(chapter_title),
    "real chapter title should not be dropped"
  );
}

#[test]
fn keeps_sentence_mentioning_section_uppercase() {
  // A sentence that happens to start with SECTION but lacks the wide
  // gap shouldn't be reclassified as a running header.
  let prose = "SECTION 3 lists the operators in detail.";
  assert!(
    !is_running_header_or_footer_line(prose),
    "narrow-spaced prose should not be dropped"
  );
}

#[test]
fn keeps_regular_toc_rows_with_page_numbers() {
  let line = "  4.16       Starting a new triangle in a free-form Gouraud-shaded triangle mesh   316";
  assert!(
    !is_running_header_or_footer_line(line),
    "expected TOC row to stay, got: {line}"
  );
}

#[test]
fn removes_duplicate_centered_heading_lines() {
  let input = concat!(
    "                Figures\n",
    "  9.9         Rendering of the 3D artwork using View0 (no cross section)   824\n",
    "                                                                                                           Figures\n",
    "  9.10       Rendering of the 3D artwork using View1 (cross section perpendicular to the \n"
  );

  let output = sanitize_layout_text(input);
  assert_eq!(
    output.matches("Figures").count(),
    1,
    "expected duplicate centered heading to be removed, got: {output:?}"
  );
}

#[test]
fn detects_supported_centered_heading_labels() {
  assert_eq!(
    centered_heading_label("                Figures"),
    Some("Figures")
  );
  assert_eq!(
    centered_heading_label("                Contents"),
    Some("Contents")
  );
  assert_eq!(centered_heading_label("Body heading"), None);
}

#[test]
fn removes_private_use_icon_only_lines() {
  let input = concat!("Before\n", "  \u{f05a}\n", "After\n",);

  let output = sanitize_layout_text(input);
  assert!(
    !output.contains('\u{f05a}'),
    "expected private-use icon to be removed, got: {output:?}"
  );
  assert!(output.contains("Before"));
  assert!(output.contains("After"));
}

#[test]
fn removes_private_use_icons_from_inline_callouts() {
  let input = "  \u{f0eb}        Helpful tip text\n";
  let normalized = normalize_extracted_line(input);
  assert!(
    !normalized.contains('\u{f0eb}'),
    "expected inline private-use icon to be removed, got: {normalized:?}"
  );
  assert!(
    normalized.contains("Helpful tip text"),
    "expected remaining callout text to be preserved, got: {normalized:?}"
  );
}

#[test]
fn normalizes_nbsp_to_ascii_space() {
  let input = "A\u{00a0}B\n";
  let output = sanitize_layout_text(input);
  assert!(
    output.contains("A B"),
    "expected nbsp to normalize to plain space, got: {output:?}"
  );
}
