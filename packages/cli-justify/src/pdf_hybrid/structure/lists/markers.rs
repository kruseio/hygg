use crate::text_utils::{char_len, leading_whitespace};

use crate::pdf_hybrid::structure::layout_signals::looks_like_git_log_graph_line;
use crate::pdf_hybrid::structure::should_keep_pdf_line_layout;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ListMarkerKind {
  Bullet,
  OptionFlag,
  FormatSpecifier,
  Numbered,
}

pub(crate) fn parse_list_marker_with_kind(
  line: &str,
) -> Option<(String, String, String, ListMarkerKind)> {
  let indent = leading_whitespace(line).to_string();
  let trimmed = line.trim_start_matches([' ', '\t']);

  // `git log --graph` rows start with `*` too but mean something different
  // — bail out before the `*` bullet branch below so the whole graph
  // stays a single code block instead of getting reformatted line-by-
  // line as a bullet list.
  if looks_like_git_log_graph_line(trimmed) {
    return None;
  }

  for bullet in ["•", "-", "*", "◦"] {
    let marker = format!("{bullet} ");
    if let Some(rest) = trimmed.strip_prefix(&marker) {
      return Some((
        indent,
        marker,
        rest.trim().to_string(),
        ListMarkerKind::Bullet,
      ));
    }
  }

  // Option-flag rows in command-line documentation tables: `-p Description`
  // or `--name-only Description`. These look like prose to should_start_
  // new_pdf_paragraph (the indent change to a 1-char bump doesn't trigger
  // a break), so without recognising them as list items the whole table
  // collapses into one flowed paragraph.
  if let Some(flag) = parse_option_flag(trimmed) {
    let rest = &trimmed[flag.len()..];
    if let Some(rest) = rest.strip_prefix(' ') {
      let trimmed_rest = rest.trim_start();
      if !trimmed_rest.is_empty() {
        let marker = format!("{flag} ");
        return Some((
          indent,
          marker,
          trimmed_rest.to_string(),
          ListMarkerKind::OptionFlag,
        ));
      }
    }
  }

  // Printf-style format-specifier rows in command-line documentation
  // tables: `%H Commit hash`, `%an Author name`. Same problem as option
  // flags above — without explicit row recognition the whole specifier
  // table flows together as one paragraph.
  if let Some(spec) = parse_format_specifier(trimmed) {
    let rest = &trimmed[spec.len()..];
    if let Some(rest) = rest.strip_prefix(' ') {
      let trimmed_rest = rest.trim_start();
      if !trimmed_rest.is_empty() {
        let marker = format!("{spec} ");
        return Some((
          indent,
          marker,
          trimmed_rest.to_string(),
          ListMarkerKind::FormatSpecifier,
        ));
      }
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
  let delimiter = chars.next()?;
  if delimiter != '.' && delimiter != ')' {
    return None;
  }
  let space = chars.next()?;
  if space != ' ' {
    return None;
  }

  let marker = format!("{}{} ", &trimmed[..idx], delimiter);
  let content = chars.as_str().trim().to_string();
  Some((indent, marker, content, ListMarkerKind::Numbered))
}

pub(crate) fn parse_list_marker(
  line: &str,
) -> Option<(String, String, String)> {
  parse_list_marker_with_kind(line)
    .map(|(indent, marker, content, _kind)| (indent, marker, content))
}

/// Recognises an option-flag token at the start of `trimmed` and returns
/// the slice covering the flag (without the trailing space). Matches:
///
///   * `-X[X...]` — short flags, at least one letter after the dash.
///   * `--XX[X...]` — long flags, at least one letter after the two dashes.
///
/// In both forms the flag name may include digits and additional ASCII
/// hyphens after the leading letter (e.g. `--name-status`, `--abbrev-1`),
/// but must START with a letter so plain negative numbers like `-3` and
/// `--` are not mistaken for flags.
fn parse_option_flag(trimmed: &str) -> Option<&str> {
  let bytes = trimmed.as_bytes();
  if bytes.first() != Some(&b'-') {
    return None;
  }
  let dash_end = if bytes.get(1) == Some(&b'-') { 2 } else { 1 };
  let first_name = *bytes.get(dash_end)?;
  if !first_name.is_ascii_alphabetic() {
    return None;
  }
  let mut end = dash_end + 1;
  while end < bytes.len() {
    let ch = bytes[end];
    if ch.is_ascii_alphanumeric() || ch == b'-' {
      end += 1;
    } else {
      break;
    }
  }
  Some(&trimmed[..end])
}

/// Recognises a printf-style format specifier at the start of `trimmed`
/// (e.g. `%H`, `%an`, `%cd`) and returns the slice covering it (without
/// the trailing space). Requires at least one letter immediately after
/// the `%` so bare percent signs or stray `% Foo` text mid-prose don't
/// false-match.
fn parse_format_specifier(trimmed: &str) -> Option<&str> {
  let bytes = trimmed.as_bytes();
  if bytes.first() != Some(&b'%') {
    return None;
  }
  let first_name = *bytes.get(1)?;
  if !first_name.is_ascii_alphabetic() {
    return None;
  }
  let mut end = 2;
  while end < bytes.len() {
    let ch = bytes[end];
    if ch.is_ascii_alphanumeric() {
      end += 1;
    } else {
      break;
    }
  }
  Some(&trimmed[..end])
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
