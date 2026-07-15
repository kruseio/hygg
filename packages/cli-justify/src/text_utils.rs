pub(crate) fn split_at_char(s: &str, n: usize) -> (&str, Option<&str>) {
  for (char_index, (i, _)) in s.char_indices().enumerate() {
    if char_index == n {
      let (w1, w2) = s.split_at(i);
      return (w1, Some(w2));
    }
  }

  (s, None)
}

pub(crate) fn char_len(s: &str) -> usize {
  s.chars().count()
}

pub(crate) fn is_ascii_numeric(s: &str) -> bool {
  !s.is_empty() && s.chars().all(|ch| ch.is_ascii_digit())
}

pub(crate) fn split_at_last_whitespace_before(
  s: &str,
  max_chars: usize,
) -> Option<usize> {
  let mut last_ws_byte_idx: Option<usize> = None;
  for (char_index, (byte_idx, ch)) in s.char_indices().enumerate() {
    if char_index >= max_chars {
      break;
    }
    if ch == ' ' || ch == '\t' {
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
  char_len(leading_whitespace(s))
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
