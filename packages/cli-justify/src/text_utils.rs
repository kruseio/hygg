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
  // A tab can advance up to eight columns at the usual terminal tab stops.
  // Measure the spans separately so unicode-width's own tab width is not
  // counted in addition to that conservative estimate.
  let mut width = 0;
  for (index, span) in s.split('\t').enumerate() {
    if index > 0 {
      width += 8;
    }
    width += UnicodeWidthStr::width(span);
  }
  width
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

  // `last_token` came from `s`, so this search always succeeds.
  let number_start = s.rfind(last_token).expect("last token is in source");
  let before_number = &s[..number_start];
  let gap_before_number =
    before_number.chars().rev().take_while(|ch| ch.is_whitespace()).count();
  if gap_before_number < min_gap {
    return (s, None);
  }

  (before_number.trim_end(), Some(last_token))
}

#[cfg(test)]
mod tests {
  use super::{
    drop_one_leading_whitespace, is_ascii_numeric, leading_whitespace_width,
    split_at_last_whitespace_before, split_trailing_numeric_token_with_min_gap,
  };

  #[test]
  fn whitespace_split_uses_columns_but_returns_a_byte_offset() {
    let text = "中文 abc def";
    assert_eq!(split_at_last_whitespace_before(text, 4), None);
    assert_eq!(split_at_last_whitespace_before(text, 5), Some(6));
    assert_eq!(split_at_last_whitespace_before(text, 9), Some(10));
    assert_eq!(
      &text[..split_at_last_whitespace_before(text, 9).unwrap()],
      "中文 abc"
    );

    assert_eq!(split_at_last_whitespace_before("\tab", 7), None);
    assert_eq!(split_at_last_whitespace_before("\tab", 8), Some(0));
  }

  #[test]
  fn leading_ascii_whitespace_is_removed_one_character_at_a_time() {
    assert_eq!(drop_one_leading_whitespace("  中文"), " 中文");
    assert_eq!(drop_one_leading_whitespace("\t中文"), "中文");
    assert_eq!(drop_one_leading_whitespace("中文"), "中文");
  }

  #[test]
  fn leading_whitespace_width_counts_tabs_conservatively() {
    assert_eq!(leading_whitespace_width("\t 中文"), 9);
    assert_eq!(leading_whitespace_width("  中文"), 2);
    assert_eq!(leading_whitespace_width("中文  "), 0);
  }

  #[test]
  fn trailing_page_number_requires_an_ascii_number_and_a_wide_gap() {
    assert!(!is_ascii_numeric(""));
    assert!(!is_ascii_numeric("１２"));
    assert!(is_ascii_numeric("12"));

    assert_eq!(split_trailing_numeric_token_with_min_gap("", 2), ("", None));
    assert_eq!(
      split_trailing_numeric_token_with_min_gap("中文  十二", 2),
      ("中文  十二", None)
    );
    assert_eq!(
      split_trailing_numeric_token_with_min_gap("中文 12", 2),
      ("中文 12", None)
    );
    assert_eq!(
      split_trailing_numeric_token_with_min_gap("中文   12", 2),
      ("中文", Some("12"))
    );
  }
}

#[cfg(test)]
mod properties;
