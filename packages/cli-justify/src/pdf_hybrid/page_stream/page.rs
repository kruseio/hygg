use crate::pdf_hybrid::engine::justify_pdf_hybrid;

// Same lookback window the per-page engine uses when reasoning about
// sibling list / caption continuity, applied here to peek across the
// page boundary in `smooth_pdf_page_seams`.
pub(crate) const MAX_SEAM_LOOKBACK_LINES: usize = 12;

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

pub(crate) fn trim_edge_blanks(lines: &mut Vec<String>) {
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

pub(crate) fn detect_partial_paragraphs(
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
