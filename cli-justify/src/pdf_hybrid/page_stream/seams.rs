use super::page::MAX_SEAM_LOOKBACK_LINES;
use crate::pdf_hybrid::structure::{
  looks_like_git_log_graph_line, looks_like_table_or_figure_caption,
  parse_list_marker,
};
use crate::text_utils::char_len;

/// Number of blank lines to insert between two adjacent PDF page outputs
/// in the streaming reader.
///
/// Returns:
///   * `0` when the two pages should read as one continuous block — a bulleted
///     / numbered list whose sibling items span the page break, or a caption
///     list (`Plate N …`, `Figure 3.4 …`, `Table 2 …`) whose entries straddle a
///     page boundary.
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
  let digit_count = rest.chars().take_while(|ch| ch.is_ascii_digit()).count();
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
