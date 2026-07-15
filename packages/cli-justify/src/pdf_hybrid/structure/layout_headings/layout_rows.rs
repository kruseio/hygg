use crate::text_utils::{
  is_ascii_numeric, leading_whitespace_width,
  split_trailing_numeric_token_with_min_gap,
};

pub(crate) fn looks_like_multi_column_row(line: &str) -> bool {
  if line.split_whitespace().count() < 3 {
    return false;
  }

  let leading_ws = leading_whitespace_width(line);
  if leading_ws <= 3 && count_internal_gaps_of_width(line, 5) >= 1 {
    return true;
  }

  // Tables, callouts, and code-aligned rows can be indented further but still
  // carry multiple wide internal gaps that we want to preserve verbatim.
  count_internal_gaps_of_width(line, 5) >= 2
}

fn count_internal_gaps_of_width(line: &str, threshold: usize) -> usize {
  let trimmed = line.trim_start_matches([' ', '\t']);
  let mut count = 0usize;
  let mut run = 0usize;
  let mut seen_non_space = false;
  for ch in trimmed.chars() {
    if ch == ' ' {
      if seen_non_space {
        run += 1;
      }
      continue;
    }
    if run >= threshold {
      count += 1;
    }
    run = 0;
    seen_non_space = true;
  }
  count
}

fn has_wide_gap_before_page_number(trimmed: &str) -> bool {
  let (label, page_number) =
    split_trailing_numeric_token_with_min_gap(trimmed, 4);
  if page_number.is_none() {
    return false;
  }
  if label.is_empty() {
    return false;
  }

  let label_words = label.split_whitespace().count();
  let label_chars = label.chars().count();
  label_words <= 8 && label_chars <= 64
}

pub(crate) fn looks_like_page_header_or_footer(line: &str) -> bool {
  let trimmed = line.trim();
  if trimmed.is_empty() {
    return false;
  }
  let tokens: Vec<&str> = trimmed.split_whitespace().collect();
  let short_numeric_suffix = tokens.len() <= 4
    && tokens.last().is_some_and(|token| is_ascii_numeric(token));

  if is_ascii_numeric(trimmed) {
    return true;
  }

  let leading_ws = leading_whitespace_width(line);
  if leading_ws >= 16 && short_numeric_suffix {
    return true;
  }

  if has_wide_gap_before_page_number(trimmed) {
    return true;
  }

  short_numeric_suffix
    && trimmed.len() <= 40
    && tokens
      .first()
      .is_some_and(|token| token.chars().all(|ch| ch.is_alphabetic()))
}
