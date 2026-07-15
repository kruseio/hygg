use super::page::{
  detect_partial_paragraphs, justify_pdf_page, justify_pdf_seam,
};
use super::seams::inter_page_blank_count;

#[test]
fn detects_tail_partial_when_paragraph_lacks_terminator() {
  let text = "First paragraph ends cleanly.\n\nThis longer paragraph carries over without any punctuation at the end";
  let (head, tail) = detect_partial_paragraphs(text);
  assert!(
    head.is_none(),
    "first paragraph starts uppercase, not a continuation"
  );
  let tail = tail.expect("trailing partial should be detected");
  assert!(tail.contains("without any punctuation"));
}

#[test]
fn detects_head_partial_when_first_paragraph_starts_lowercase() {
  let text = "continuation of the prior page's sentence finishing here.\n\nA new paragraph begins.";
  let (head, _tail) = detect_partial_paragraphs(text);
  let head = head.expect("leading partial should be detected");
  assert!(head.starts_with("continuation"));
}

#[test]
fn ignores_short_trailing_heading() {
  let text = "Some body text ends here.\n\nSummary";
  let (_head, tail) = detect_partial_paragraphs(text);
  assert!(tail.is_none(), "short final fragment is treated as heading");
}

#[test]
fn justify_pdf_page_reports_line_counts() {
  let raw = "first body paragraph stays on page.\n\ntext that continues forward without a period at the end";
  let p = justify_pdf_page(raw, 30);
  assert!(p.tail_partial.is_some());
  let tail = p.tail_partial.unwrap();
  assert!(tail.line_count >= 1);
  assert!(tail.line_count <= p.lines.len());
}

#[test]
fn justify_pdf_seam_merges_into_one_paragraph() {
  let prev = "the quick brown fox jumps over";
  let next = "the lazy dog and goes home.";
  let merged = justify_pdf_seam(prev, next, 80);
  let joined = merged.join(" ");
  assert!(
    joined.contains("over the lazy dog"),
    "seam should join into one paragraph: {merged:?}"
  );
}

#[test]
fn justify_pdf_page_strips_leading_and_trailing_blanks() {
  // Per-page raw text typically starts with a paragraph-break blank
  // (pdf_oxide y-gap heuristic firing before the first content row)
  // and ends with a blank produced by `text.split('\n')` on a `\n`-
  // terminated page. Neither should survive into `standalone_lines`
  // — they exist only as concatenation artifacts.
  let raw = "\n• First bullet on this page.\n• Second bullet.\n";
  let p = justify_pdf_page(raw, 80);
  assert!(
    p.lines.first().is_some_and(|l| !l.is_empty()),
    "leading blank should be stripped, got: {:?}",
    p.lines
  );
  assert!(
    p.lines.last().is_some_and(|l| !l.is_empty()),
    "trailing blank should be stripped, got: {:?}",
    p.lines
  );
}

#[test]
fn inter_page_blank_count_drops_blanks_between_sibling_bullets() {
  // Two pages each carry one bullet from the same logical list.
  // The boundary should read as one continuous block (0 blanks).
  let this = vec!["• Chapter 7, Transparency.".to_string()];
  let next = vec!["• Chapter 8, Interactive Features.".to_string()];
  assert_eq!(inter_page_blank_count(&this, &next), 0);
}

#[test]
fn inter_page_blank_count_drops_blanks_between_sibling_bullets_with_continuation()
 {
  // The trailing line of the previous page is a wrapped continuation
  // of a bullet, not the bullet header. We must still recognise the
  // sibling relationship by walking back through continuation lines.
  let this = vec![
    "• Chapter 7, Transparency, discusses the operation".to_string(),
    "  of the transparent imaging model.".to_string(),
  ];
  let next = vec!["• Chapter 8, Interactive Features.".to_string()];
  assert_eq!(inter_page_blank_count(&this, &next), 0);
}

#[test]
fn inter_page_blank_count_drops_blanks_between_captions() {
  let this = vec!["Plate 14 Radial shading effect (page 313)".to_string()];
  let next = vec!["Plate 15 Coons patch mesh (page 321)".to_string()];
  assert_eq!(inter_page_blank_count(&this, &next), 0);
}

#[test]
fn inter_page_blank_count_drops_blanks_between_captions_via_wrap_tail() {
  // The previous page's last line is the wrap tail of a caption
  // (`page 313)`), not the caption header. Walk back to find the
  // header (`Plate 17 …`).
  let this = vec![
    "Plate 17 Isolated and knockout groups (Sections 7.3.4, page".to_string(),
    "539 and 7.3.5, page 540)".to_string(),
  ];
  let next = vec!["Plate 18 RGB blend modes (page 520)".to_string()];
  assert_eq!(inter_page_blank_count(&this, &next), 0);
}

#[test]
fn inter_page_blank_count_keeps_one_blank_between_unrelated_paragraphs() {
  let this = vec!["End of one prose paragraph on the prior page.".to_string()];
  let next =
    vec!["Start of a new prose paragraph on the next page.".to_string()];
  assert_eq!(inter_page_blank_count(&this, &next), 1);
}

#[test]
fn inter_page_blank_count_keeps_one_blank_when_list_ends_and_prose_starts() {
  let this = vec!["• Final list item on prior page.".to_string()];
  let next = vec!["A fresh prose paragraph on the next page.".to_string()];
  assert_eq!(inter_page_blank_count(&this, &next), 1);
}

#[test]
fn inter_page_blank_count_drops_blanks_between_git_graph_rows() {
  let this = vec![
    "  * 2d3acf9 Ignore errors from SIGCHLD on trap".to_string(),
    "  * | 30e367c Timeout code and tests".to_string(),
  ];
  let next = vec!["  * | 5a09431 Add timeout protection to grit".to_string()];
  assert_eq!(inter_page_blank_count(&this, &next), 0);
}
