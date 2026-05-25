use crate::text_utils::{char_len, leading_whitespace};

use super::layout_signals::looks_like_git_log_graph_line;
use super::should_keep_pdf_line_layout;

pub(crate) fn parse_list_marker(
  line: &str,
) -> Option<(String, String, String)> {
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
      return Some((indent, marker, rest.trim().to_string()));
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
        return Some((indent, marker, trimmed_rest.to_string()));
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
        return Some((indent, marker, trimmed_rest.to_string()));
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
  Some((indent, marker, content))
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

/// Recognises labelled figure / table / plate captions like
///   * `Table 2. Common options to git log`
///   * `Figure 3.1: Anatomy of a commit`
///   * `Table 12 — Numeric type ranges`
///   * `Plate 14 Radial shading effect …`
///
/// These are typographically separate from the surrounding prose in the
/// PDF (italic / bold / extra leading), but pdf_extract gives us only the
/// text. Without forcing a paragraph break here the caption collapses
/// into the trailing sentence of the previous paragraph — and so does the
/// table that follows it. The Plate variant also covers the
/// list-of-plates section in front-matter, where each entry sits on its
/// own line and would otherwise get glued into one re-justified
/// paragraph.
pub(crate) fn looks_like_table_or_figure_caption(trimmed: &str) -> bool {
  let mut words = trimmed.split_whitespace();
  let Some(label) = words.next() else {
    return false;
  };
  if !matches!(label, "Table" | "Figure" | "Plate" | "Diagram") {
    return false;
  }
  let Some(number) = words.next() else {
    return false;
  };
  let number_clean = number.trim_end_matches(['.', ':', ')']);
  if number_clean.is_empty() {
    return false;
  }
  if !number_clean.chars().all(|ch| ch.is_ascii_digit() || ch == '.') {
    return false;
  }
  // Need at least one more token so we don't fire on a bare "Table 2."
  // reference sitting at the end of an unrelated sentence.
  words.next().is_some()
}

pub(crate) fn should_start_new_pdf_paragraph(
  current_indent: &str,
  previous_line: &str,
  line: &str,
) -> bool {
  // Table / figure captions are paragraph-level labels for the figure
  // that follows. Whatever the indent comparison says, treat them as
  // their own paragraph so the caption (and the table beneath it) don't
  // get glued onto the prior sentence.
  if looks_like_table_or_figure_caption(line.trim()) {
    return true;
  }

  let next_indent = leading_whitespace(line);
  if next_indent == current_indent {
    // Same indent ordinarily means "continuation of the same paragraph",
    // but a sequence of `( ... )` PDF literal-string examples breaks that
    // assumption: each example shares the block's indent yet is meant to
    // be displayed as its own item, not glued into one wrapped paragraph.
    // Detect this shape — the previous line in the pending paragraph
    // closes a parenthesised expression and the new line opens a fresh
    // one with the PDF rendering's leading-space convention — and split
    // there instead.
    let prev = previous_line.trim_end();
    let next_trimmed = line.trim_start();
    if prev.ends_with(')') && next_trimmed.starts_with("( ") {
      return true;
    }
    // Multi-line literal-string examples — `( These \` continued by
    // `two strings \` and closed by `are the same . )` on the next
    // line, or `( ... .` closed by a bare `)` underneath — must keep
    // each source line on its own output line. Break the paragraph
    // when the new line opens or closes a `( ... )` example, or when
    // the previous line ended with a backslash continuation.
    if next_trimmed.starts_with("( ")
      || matches!(next_trimmed, "(" | ")" | "( )")
    {
      return true;
    }
    if prev.ends_with('\\') {
      return true;
    }
    return false;
  }

  let current_indent_width = char_len(current_indent);
  let next_indent_width = char_len(next_indent);
  if next_indent_width > current_indent_width {
    let prev = previous_line.trim_end();
    // A trailing colon ends the previous thought just like a period: the
    // line that follows it ("Examples follow:", "Note the following:") is
    // virtually never a continuation of the same sentence, even when it
    // happens to start with '(' or a lowercase letter. Without this the
    // indent-bump check below would glue a code-like example such as
    // `( This is a string )` onto the introductory text.
    if !prev.is_empty() && !prev.ends_with(['.', '?', '!', ':']) {
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
