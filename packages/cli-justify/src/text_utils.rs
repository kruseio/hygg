use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

// Split only at grapheme boundaries. If one grapheme exceeds the budget,
// emit it intact so even a one-column CJK layout always makes progress.
pub(crate) fn split_at_width(s: &str, columns: usize) -> (&str, Option<&str>) {
  let mut width = 0;
  for (byte_idx, grapheme) in s.grapheme_indices(true) {
    let next_width = width + display_width(grapheme);
    if next_width > columns && byte_idx > 0 {
      let (left, right) = s.split_at(byte_idx);
      return (left, Some(right));
    }
    width = next_width;
  }
  (s, None)
}

pub(crate) fn display_width(s: &str) -> usize {
  // unicode-width treats tabs as zero-width control characters. A tab can
  // advance up to eight columns at the usual terminal tab stops, so count
  // that upper bound when the starting column is unknown.
  UnicodeWidthStr::width(s)
    + s.bytes().filter(|byte| *byte == b'\t').count() * 8
}

pub(crate) fn is_ascii_numeric(s: &str) -> bool {
  !s.is_empty() && s.chars().all(|ch| ch.is_ascii_digit())
}

pub(crate) fn split_at_last_whitespace_before(
  s: &str,
  max_columns: usize,
) -> Option<usize> {
  let mut last_ws_byte_idx: Option<usize> = None;
  let mut width = 0;
  for (byte_idx, grapheme) in s.grapheme_indices(true) {
    width += display_width(grapheme);
    if width > max_columns {
      break;
    }
    if grapheme == " " || grapheme == "\t" {
      last_ws_byte_idx = Some(byte_idx);
    }
  }
  last_ws_byte_idx
}

pub(crate) fn drop_one_leading_whitespace(s: &str) -> &str {
  let mut chars = s.chars();
  match chars.next() {
    Some(' ') | Some('\t') => chars.as_str(),
    _ => s,
  }
}

pub(crate) fn leading_whitespace(s: &str) -> &str {
  let mut end = 0usize;
  for (byte_idx, ch) in s.char_indices() {
    if ch != ' ' && ch != '\t' {
      break;
    }
    end = byte_idx + ch.len_utf8();
  }
  &s[..end]
}

pub(crate) fn leading_whitespace_width(s: &str) -> usize {
  display_width(leading_whitespace(s))
}

pub(crate) fn split_trailing_numeric_token_with_min_gap(
  s: &str,
  min_gap: usize,
) -> (&str, Option<&str>) {
  let Some(last_token) = s.split_whitespace().last() else {
    return (s, None);
  };
  if !is_ascii_numeric(last_token) {
    return (s, None);
  }

  let Some(number_start) = s.rfind(last_token) else {
    return (s, None);
  };
  let before_number = &s[..number_start];
  let gap_before_number =
    before_number.chars().rev().take_while(|ch| ch.is_whitespace()).count();
  if gap_before_number < min_gap {
    return (s, None);
  }

  (before_number.trim_end(), Some(last_token))
}
