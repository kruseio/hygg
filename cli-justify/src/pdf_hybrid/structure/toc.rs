use crate::text_utils::{
  char_len, is_ascii_numeric, leading_whitespace, leading_whitespace_width,
  split_trailing_numeric_token_with_min_gap,
};

use super::looks_like_toc_entry;
use super::toc_patterns::{
  looks_like_caption_prefix, looks_like_named_toc_heading,
  looks_like_toc_entry_prefix, looks_like_toc_section_marker,
  merge_counter_into_prefix_if_needed,
};

pub(crate) struct AlignedTocRow {
  pub(crate) indent: String,
  pub(crate) entry_prefix: String,
  pub(crate) title: String,
  pub(crate) page_number: String,
}

pub(crate) struct AlignedTocRowStart {
  pub(crate) indent: String,
  pub(crate) entry_prefix: String,
  pub(crate) title_fragment: String,
  pub(crate) page_number: Option<String>,
}

fn split_on_first_wide_gap(text: &str) -> Option<(&str, &str)> {
  let mut gap_start: Option<usize> = None;
  let mut gap_len = 0usize;

  for (idx, ch) in text.char_indices() {
    if ch.is_whitespace() {
      if gap_start.is_none() {
        gap_start = Some(idx);
      }
      gap_len += 1;
      continue;
    }

    if gap_start.is_some() && gap_len >= 2 {
      let prefix = &text[..idx];
      let title = text[idx..].trim();
      if !prefix.trim().is_empty() && !title.is_empty() {
        return Some((prefix, title));
      }
    }

    gap_start = None;
    gap_len = 0;
  }

  None
}

fn parse_dot_leader_toc_row(line: &str) -> Option<AlignedTocRowStart> {
  let trimmed = line.trim();
  let page_number = trimmed.split_whitespace().last()?;
  if !is_ascii_numeric(page_number) {
    return None;
  }

  let number_start = trimmed.rfind(page_number)?;
  let before_number =
    trimmed[..number_start].trim_end_matches(|ch: char| ch.is_whitespace());

  let mut leader_start = before_number.len();
  let mut leader_dot_count = 0usize;
  for (idx, ch) in before_number.char_indices().rev() {
    if ch == '.' || ch.is_whitespace() {
      if ch == '.' {
        leader_dot_count += 1;
      }
      leader_start = idx;
      continue;
    }
    break;
  }

  if leader_dot_count < 4 {
    return None;
  }

  let title = before_number[..leader_start].trim_end();
  if title.is_empty() {
    return None;
  }

  Some(AlignedTocRowStart {
    indent: leading_whitespace(line).to_string(),
    entry_prefix: String::new(),
    title_fragment: title.to_string(),
    page_number: Some(page_number.to_string()),
  })
}

pub(crate) fn parse_aligned_toc_row_start(
  line: &str,
) -> Option<AlignedTocRowStart> {
  let trimmed = line.trim();
  if trimmed.is_empty() {
    return None;
  }
  if looks_like_toc_entry(trimmed) {
    return parse_dot_leader_toc_row(line);
  }

  let (left, page_number) =
    split_trailing_numeric_token_with_min_gap(trimmed, 2);
  let (entry_prefix, title) = split_on_first_wide_gap(left)?;
  let (entry_prefix, title) =
    merge_counter_into_prefix_if_needed(entry_prefix, title)?;
  let prefix_trimmed = entry_prefix.trim_start();
  if looks_like_caption_prefix(prefix_trimmed) {
    return None;
  }

  if page_number.is_none() {
    let indent_width = leading_whitespace_width(line);
    let gap_width =
      entry_prefix.chars().rev().take_while(|ch| ch.is_whitespace()).count();
    if indent_width > 8
      || gap_width < 3
      || !looks_like_toc_entry_prefix(&entry_prefix)
    {
      return None;
    }
  }

  if entry_prefix.trim_end().chars().count() > 24
    || !looks_like_toc_entry_prefix(&entry_prefix)
  {
    return None;
  }

  let indent = leading_whitespace(line).to_string();
  Some(AlignedTocRowStart {
    indent,
    entry_prefix,
    title_fragment: title,
    page_number: page_number.map(str::to_string),
  })
}

pub(crate) fn parse_aligned_toc_continuation(
  line: &str,
) -> Option<(String, Option<String>)> {
  let trimmed = line.trim();
  if trimmed.is_empty() {
    return None;
  }

  let (left, page_number) =
    split_trailing_numeric_token_with_min_gap(trimmed, 2);
  if left.is_empty() {
    return None;
  }

  Some((left.to_string(), page_number.map(str::to_string)))
}

pub(crate) fn parse_plain_aligned_toc_row(line: &str) -> Option<AlignedTocRow> {
  let trimmed = line.trim();
  if trimmed.is_empty() || looks_like_toc_entry(trimmed) {
    return None;
  }

  let (title, page_number) =
    split_trailing_numeric_token_with_min_gap(trimmed, 2);
  let Some(page_number) = page_number else {
    return None;
  };
  if title.is_empty() {
    return None;
  }

  let first_token = title.split_whitespace().next()?;
  if looks_like_toc_section_marker(first_token) {
    return None;
  }
  if looks_like_named_toc_heading(title) {
    return None;
  }
  if !first_token.chars().next().is_some_and(|ch| ch.is_alphabetic()) {
    return None;
  }

  Some(AlignedTocRow {
    indent: leading_whitespace(line).to_string(),
    entry_prefix: String::new(),
    title: title.to_string(),
    page_number: page_number.to_string(),
  })
}

pub(crate) fn normalize_preserved_compact_layout_line(line: &str) -> String {
  let indent = leading_whitespace(line);
  let indent_width = char_len(indent);
  if indent_width > 3 {
    return line.to_string();
  }

  let trimmed = line.trim_start_matches([' ', '\t']);
  let Some(label_end) = trimmed.find(char::is_whitespace) else {
    return line.to_string();
  };
  let label = &trimmed[..label_end];
  if label.chars().count() > 12
    || !label.chars().next().is_some_and(|ch| ch.is_uppercase())
    || !label.chars().all(|ch| ch.is_alphabetic())
  {
    return line.to_string();
  }

  let after_label = &trimmed[label_end..];
  let label_gap_width =
    after_label.chars().take_while(|ch| ch.is_whitespace()).count();
  if label_gap_width == 0 {
    return line.to_string();
  }
  let after_label = &after_label[label_gap_width..];

  let mut number_end = 0usize;
  for ch in after_label.chars() {
    if ch.is_ascii_digit() {
      number_end += ch.len_utf8();
    } else {
      break;
    }
  }
  if number_end == 0 {
    return line.to_string();
  }

  let number = &after_label[..number_end];
  let remainder = &after_label[number_end..];
  let spacing_len = remainder.chars().take_while(|&ch| ch == ' ').count();
  if spacing_len < 2 {
    return line.to_string();
  }

  let text = remainder[spacing_len..].trim_start();
  if text.is_empty() {
    return line.to_string();
  }

  let marker = format!("{label} {number}");
  let marker_width = char_len(&marker);
  let target_title_column = 14usize;
  let target_gap_width =
    target_title_column.saturating_sub(indent_width + marker_width + 1);
  if target_gap_width < 2 {
    return line.to_string();
  }

  format!("{indent}{marker}{}{text}", " ".repeat(target_gap_width))
}
