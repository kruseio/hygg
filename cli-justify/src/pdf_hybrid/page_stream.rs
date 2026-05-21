use super::engine::justify_pdf_hybrid;

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
  let lines = justify_pdf_hybrid(raw_text, col);
  let (head_raw, tail_raw) = detect_partial_paragraphs(raw_text);

  let head_partial = head_raw.map(|raw| {
    let line_count = justify_pdf_hybrid(&raw, col).len();
    PartialParagraph { raw_text: raw, line_count }
  });
  let tail_partial = tail_raw.map(|raw| {
    let line_count = justify_pdf_hybrid(&raw, col).len();
    PartialParagraph { raw_text: raw, line_count }
  });

  PdfPageJustified { lines, head_partial, tail_partial }
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
  if prev.is_empty() {
    return justify_pdf_hybrid(next, col);
  }
  if next.is_empty() {
    return justify_pdf_hybrid(prev, col);
  }
  let joined = format!("{prev}\n{next}");
  justify_pdf_hybrid(&joined, col)
}

fn detect_partial_paragraphs(
  raw_text: &str,
) -> (Option<String>, Option<String>) {
  let paragraphs = split_paragraphs(raw_text);
  if paragraphs.is_empty() {
    return (None, None);
  }

  let head = if looks_like_continuation(paragraphs.first().copied().unwrap_or(""))
  {
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
  matches!(first_word.to_ascii_lowercase().as_str(), "and" | "but" | "or" | "so")
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
    assert!(head.is_none(), "first paragraph starts uppercase, not a continuation");
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
    assert!(joined.contains("over the lazy dog"), "seam should join into one paragraph: {merged:?}");
  }
}
