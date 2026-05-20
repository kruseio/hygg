use crate::text_utils::{char_len, leading_whitespace};

use super::should_keep_pdf_line_layout;

pub(crate) fn parse_list_marker(
  line: &str,
) -> Option<(String, String, String)> {
  let indent = leading_whitespace(line).to_string();
  let trimmed = line.trim_start_matches([' ', '\t']);

  for bullet in ["•", "-", "*", "◦"] {
    let marker = format!("{bullet} ");
    if let Some(rest) = trimmed.strip_prefix(&marker) {
      return Some((indent, marker, rest.trim().to_string()));
    }
  }

  let mut idx = 0usize;
  for ch in trimmed.chars() {
    if !ch.is_ascii_digit() {
      break;
    }
    idx += ch.len_utf8();
  }
  if idx == 0 {
    return None;
  }

  let remainder = &trimmed[idx..];
  let mut chars = remainder.chars();
  let Some(delimiter) = chars.next() else {
    return None;
  };
  if delimiter != '.' && delimiter != ')' {
    return None;
  }
  let Some(space) = chars.next() else {
    return None;
  };
  if space != ' ' {
    return None;
  }

  let marker = format!("{}{} ", &trimmed[..idx], delimiter);
  let content = chars.as_str().trim().to_string();
  Some((indent, marker, content))
}

pub(crate) fn is_list_continuation_line(
  line: &str,
  list_indent: &str,
  marker: &str,
) -> bool {
  let trimmed = line.trim();
  if trimmed.is_empty() {
    return false;
  }
  if parse_list_marker(line).is_some() {
    return false;
  }
  if should_keep_pdf_line_layout(line) {
    return false;
  }

  let leading_ws =
    line.chars().take_while(|&ch| ch == ' ' || ch == '\t').count();
  let list_indent_width = char_len(list_indent);
  let continuation_indent_width = list_indent_width + char_len(marker);
  if leading_ws >= continuation_indent_width {
    return true;
  }

  leading_ws >= list_indent_width
    && trimmed.chars().next().is_some_and(|ch| ch.is_lowercase())
}

pub(crate) fn should_start_new_pdf_paragraph(
  current_indent: &str,
  previous_line: &str,
  line: &str,
) -> bool {
  let next_indent = leading_whitespace(line);
  if next_indent == current_indent {
    return false;
  }

  let current_indent_width = char_len(current_indent);
  let next_indent_width = char_len(next_indent);
  if next_indent_width > current_indent_width {
    let prev = previous_line.trim_end();
    if !prev.is_empty() && !prev.ends_with(['.', '?', '!']) {
      let next_trimmed = line.trim_start_matches([' ', '\t']);
      if next_trimmed.is_empty() {
        return true;
      }

      let first = next_trimmed.chars().next().unwrap_or(' ');
      let looks_like_continuation_fragment = first.is_lowercase()
        || matches!(
          first,
          '('
            | ')'
            | ']'
            | '}'
            | ','
            | '.'
            | ':'
            | ';'
            | '!'
            | '?'
            | '-'
            | '—'
            | '–'
            | '/'
            | '\\'
            | '~'
        )
        || next_trimmed.chars().count() <= 4;

      if looks_like_continuation_fragment {
        return false;
      }

      // A small indent bump (1-2 chars) where the previous line ends mid-
      // sentence is almost always a wrapped continuation, often because
      // an inline code or styled run sits on the next visual line with a
      // slightly different left edge.
      let indent_bump = next_indent_width - current_indent_width;
      if indent_bump <= 2 {
        return false;
      }
    }
  }

  true
}
