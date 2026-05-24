use super::engine::justify_pdf_hybrid;
use super::structure::{
  looks_like_git_log_graph_line, looks_like_table_or_figure_caption,
  parse_list_marker,
};
use crate::text_utils::char_len;

// Same lookback window the per-page engine uses when reasoning about
// sibling list / caption continuity, applied here to peek across the
// page boundary in `smooth_pdf_page_seams`.
const MAX_SEAM_LOOKBACK_LINES: usize = 12;

/// Justified output for a single PDF page, augmented with the raw text of
/// any partial paragraphs that may continue onto a neighbouring page.
pub struct PdfPageJustified {
  pub lines: Vec<String>,
  pub head_partial: Option<PartialParagraph>,
  pub tail_partial: Option<PartialParagraph>,
}

/// A paragraph (or paragraph fragment) at a page boundary together with the
/// number of justified output lines it occupies when laid out alone. The
/// `line_count` lets a consumer splice a re-justified seam back into the
/// page's `lines` by replacing exactly the partial's worth of lines.
pub struct PartialParagraph {
  pub raw_text: String,
  pub line_count: usize,
}

pub fn justify_pdf_page(raw_text: &str, col: usize) -> PdfPageJustified {
  // Strip leading and trailing blanks so adjacent pages don't compound
  // their boundary blanks at concatenation time. A page's leading blank
  // is always an artifact of pdf_oxide's paragraph-break detector firing
  // before the first content row; a trailing blank is always the empty
  // element produced by `text.split('\n')` on a raw page that ends with
  // `\n`. Neither carries meaning across the page boundary. With them
  // stripped, the seam between pages is inserted by `flat_lines` (1
  // blank for an ordinary paragraph break, 0 for sibling list / caption
  // continuity) and both `rendered_line_count` and `flat_lines` agree
  // on the per-page contribution.
  let mut lines = justify_pdf_hybrid(raw_text, col);
  trim_edge_blanks(&mut lines);
  let (head_raw, tail_raw) = detect_partial_paragraphs(raw_text);

  let head_partial = head_raw.map(|raw| {
    let mut head_lines = justify_pdf_hybrid(&raw, col);
    trim_edge_blanks(&mut head_lines);
    PartialParagraph { raw_text: raw, line_count: head_lines.len() }
  });
  let tail_partial = tail_raw.map(|raw| {
    let mut tail_lines = justify_pdf_hybrid(&raw, col);
    trim_edge_blanks(&mut tail_lines);
    PartialParagraph { raw_text: raw, line_count: tail_lines.len() }
  });

  PdfPageJustified { lines, head_partial, tail_partial }
}

fn trim_edge_blanks(lines: &mut Vec<String>) {
  while lines.last().is_some_and(String::is_empty) {
    lines.pop();
  }
  while lines.first().is_some_and(String::is_empty) {
    lines.remove(0);
  }
}

/// Re-justify a seam paragraph formed by joining the trailing partial of one
/// page with the leading partial of the next. The joined text is fed through
/// the standard PDF justifier so soft hyphens, mid-word breaks and similar
/// cross-line repairs happen as if the paragraph had never been split.
pub fn justify_pdf_seam(
  prev_tail_raw: &str,
  next_head_raw: &str,
  col: usize,
) -> Vec<String> {
  let prev = prev_tail_raw.trim_end_matches(['\n', ' ', '\t']);
  let next = next_head_raw.trim_start_matches(['\n', ' ', '\t']);
  let mut lines = if prev.is_empty() {
    justify_pdf_hybrid(next, col)
  } else if next.is_empty() {
    justify_pdf_hybrid(prev, col)
  } else {
    let joined = format!("{prev}\n{next}");
    justify_pdf_hybrid(&joined, col)
  };
  trim_edge_blanks(&mut lines);
  lines
}

/// Number of blank lines to insert between two adjacent PDF page outputs
/// in the streaming reader.
///
/// Returns:
///   * `0` when the two pages should read as one continuous block — a
///     bulleted / numbered list whose sibling items span the page break,
///     or a caption list (`Plate N …`, `Figure 3.4 …`, `Table 2 …`)
///     whose entries straddle a page boundary.
///   * `1` otherwise, as the normal paragraph separator.
///
/// Both `this_lines` and `next_lines` are the per-page `standalone_lines`
/// produced by `justify_pdf_page`, with edge blanks already stripped.
/// `flat_lines` calls this to decide the separator; `rendered_line_count`
/// calls it to keep the per-page count in sync with what `flat_lines`
/// produces, so cursor positioning and "jump to page" stay correct.
pub fn inter_page_blank_count(
  this_lines: &[String],
  next_lines: &[String],
) -> usize {
  let Some(first_next) = next_lines.iter().find(|l| !l.is_empty()) else {
    return 1;
  };

  if let Some((next_indent, next_marker, _)) = parse_list_marker(first_next)
    && prior_is_sibling_list_item(this_lines, &next_indent, &next_marker)
  {
    return 0;
  }
  if looks_like_table_or_figure_caption(first_next.trim())
    && prior_is_caption(this_lines)
  {
    return 0;
  }
  if looks_like_git_log_graph_line(first_next.trim())
    && this_lines
      .iter()
      .rev()
      .find(|l| !l.is_empty())
      .is_some_and(|l| looks_like_git_log_graph_line(l.trim()))
  {
    return 0;
  }

  1
}

fn prior_is_sibling_list_item(
  this_lines: &[String],
  indent: &str,
  marker: &str,
) -> bool {
  let continuation_indent_width = char_len(indent) + char_len(marker);
  let scan_floor = this_lines.len().saturating_sub(MAX_SEAM_LOOKBACK_LINES);
  for idx in (scan_floor..this_lines.len()).rev() {
    let line = &this_lines[idx];
    if line.is_empty() {
      return false;
    }
    if line_starts_sibling_list_item(line, indent, marker) {
      return true;
    }
    let leading_ws = line.chars().take_while(|ch| *ch == ' ').count();
    if leading_ws < continuation_indent_width {
      return false;
    }
  }
  false
}

fn prior_is_caption(this_lines: &[String]) -> bool {
  let scan_floor = this_lines.len().saturating_sub(MAX_SEAM_LOOKBACK_LINES);
  for idx in (scan_floor..this_lines.len()).rev() {
    let line = &this_lines[idx];
    if line.is_empty() {
      return false;
    }
    if looks_like_table_or_figure_caption(line.trim()) {
      return true;
    }
  }
  false
}

fn line_starts_sibling_list_item(
  line: &str,
  indent: &str,
  marker: &str,
) -> bool {
  if line.starts_with(&format!("{indent}{marker}")) {
    return true;
  }
  let Some(rest) = line.strip_prefix(indent) else {
    return false;
  };
  let Some(marker_punct) = marker.trim_end().chars().last() else {
    return false;
  };
  if marker_punct != '.' && marker_punct != ')' {
    return false;
  }
  let digit_count =
    rest.chars().take_while(|ch| ch.is_ascii_digit()).count();
  if digit_count == 0 {
    return false;
  }
  let mut after_digits = rest.chars().skip(digit_count);
  let Some(delim) = after_digits.next() else {
    return false;
  };
  if delim != marker_punct {
    return false;
  }
  matches!(after_digits.next(), Some(' '))
}

fn detect_partial_paragraphs(
  raw_text: &str,
) -> (Option<String>, Option<String>) {
  let paragraphs = split_paragraphs(raw_text);
  if paragraphs.is_empty() {
    return (None, None);
  }

  let head =
    if looks_like_continuation(paragraphs.first().copied().unwrap_or("")) {
      Some(paragraphs.first().copied().unwrap_or("").to_string())
    } else {
      None
    };
  let tail = if paragraphs.len() == 1 {
    None
  } else if looks_incomplete(paragraphs.last().copied().unwrap_or("")) {
    Some(paragraphs.last().copied().unwrap_or("").to_string())
  } else {
    None
  };

  (head, tail)
}

fn split_paragraphs(text: &str) -> Vec<&str> {
  let mut paragraphs = Vec::new();
  let mut start: Option<usize> = None;
  let mut blank_run = 0usize;
  let mut byte_pos = 0usize;

  for line in text.split_inclusive('\n') {
    let trimmed = line.trim();
    if trimmed.is_empty() {
      if let Some(s) = start.take() {
        paragraphs.push(text[s..byte_pos].trim_end_matches('\n').trim_end());
      }
      blank_run += 1;
      let _ = blank_run;
    } else {
      blank_run = 0;
      if start.is_none() {
        start = Some(byte_pos);
      }
    }
    byte_pos += line.len();
  }
  if let Some(s) = start {
    paragraphs.push(text[s..byte_pos].trim_end_matches('\n').trim_end());
  }
  paragraphs
}

fn looks_like_continuation(paragraph: &str) -> bool {
  let trimmed = paragraph.trim_start();
  let Some(first_char) = trimmed.chars().next() else {
    return false;
  };
  // Lowercase ASCII or unicode lowercase is a strong signal the paragraph
  // continues the previous page's sentence.
  if first_char.is_lowercase() {
    return true;
  }
  // A first line that starts with a small connective word (and, but, or, so)
  // and the paragraph doesn't end at a sentence boundary also looks like
  // continuation.
  let first_word = trimmed
    .split_whitespace()
    .next()
    .map(|w| w.trim_end_matches(|ch: char| !ch.is_alphabetic()))
    .unwrap_or("");
  matches!(
    first_word.to_ascii_lowercase().as_str(),
    "and" | "but" | "or" | "so"
  )
}

fn looks_incomplete(paragraph: &str) -> bool {
  let trimmed = paragraph.trim_end();
  if trimmed.is_empty() {
    return false;
  }
  let last_char = trimmed.chars().rev().find(|c| !c.is_whitespace());
  let Some(last) = last_char else {
    return false;
  };
  // Sentence-terminating punctuation -> seam is clean.
  if matches!(last, '.' | '!' | '?' | ':' | ';' | ']' | ')' | '}' | '"') {
    return false;
  }
  // Very short fragment (likely a heading) -> don't treat as incomplete.
  let word_count = trimmed.split_whitespace().count();
  if word_count <= 4 {
    return false;
  }
  true
}

#[cfg(test)]
mod tests {
  use super::*;

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
  fn inter_page_blank_count_drops_blanks_between_sibling_bullets_with_continuation() {
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
    let this =
      vec!["End of one prose paragraph on the prior page.".to_string()];
    let next = vec!["Start of a new prose paragraph on the next page.".to_string()];
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
    let next = vec![
      "  * | 5a09431 Add timeout protection to grit".to_string(),
    ];
    assert_eq!(inter_page_blank_count(&this, &next), 0);
  }
}
