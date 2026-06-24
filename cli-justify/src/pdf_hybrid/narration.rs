use crate::text_utils::{char_len, leading_whitespace};

use super::structure::{
  ListMarkerKind, code_line_continues, is_list_continuation_line,
  looks_like_code_block_line, looks_like_code_continuation_line,
  looks_like_command_prompt_line, looks_like_table_or_figure_caption,
  looks_like_toc_entry, parse_aligned_toc_continuation,
  parse_aligned_toc_row_start, parse_list_marker_with_kind,
  parse_plain_aligned_toc_row,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NarrationSkipReason {
  Code,
  Table,
  Toc,
  PageNumber,
}

// A standalone page number left in the text by PDF extraction (a header/footer
// folio). Read aloud it is a bare number dropped mid-paragraph, so narration
// skips it. Kept strict — a short line that is *only* digits, optionally with a
// "Page"/"p." prefix — so real numeric prose ("1984 was…") is not dropped.
fn looks_like_page_number(trimmed: &str) -> bool {
  let body = trimmed
    .strip_prefix("Page ")
    .or_else(|| trimmed.strip_prefix("page "))
    .or_else(|| trimmed.strip_prefix("p. "))
    .unwrap_or(trimmed)
    .trim();
  !body.is_empty() && body.len() <= 4 && body.bytes().all(|b| b.is_ascii_digit())
}

pub fn pdf_hybrid_narration_skip_mask(lines: &[String]) -> Vec<bool> {
  narration_skip_reasons(lines).into_iter().map(|r| r.is_some()).collect()
}

fn narration_skip_reasons(
  lines: &[String],
) -> Vec<Option<NarrationSkipReason>> {
  let mut reasons = vec![None; lines.len()];
  let mut shell_session_indent: Option<String> = None;
  let mut code_continuation_indent_width: Option<usize> = None;
  let mut table_context: Option<(String, String)> = None;
  let mut in_fenced_code = false;
  let mut pending_toc_row = false;
  let mut in_aligned_toc = false;

  for (idx, line) in lines.iter().enumerate() {
    let trimmed = line.trim();
    if trimmed.is_empty() {
      table_context = None;
      continue;
    }

    if let Some(base_indent_width) = code_continuation_indent_width {
      if looks_like_code_continuation_line(line, base_indent_width) {
        reasons[idx] = Some(NarrationSkipReason::Code);
        code_continuation_indent_width = if code_line_continues(trimmed) {
          Some(char_len(leading_whitespace(line)))
        } else {
          None
        };
        continue;
      }
      code_continuation_indent_width = None;
    }

    if in_fenced_code {
      reasons[idx] = Some(NarrationSkipReason::Code);
      if is_fence_line(trimmed) {
        in_fenced_code = false;
      }
      continue;
    }
    if is_fence_line(trimmed) {
      reasons[idx] = Some(NarrationSkipReason::Code);
      in_fenced_code = true;
      continue;
    }

    if looks_like_page_number(trimmed) {
      reasons[idx] = Some(NarrationSkipReason::PageNumber);
      continue;
    }

    if pending_toc_row {
      if let Some((_fragment, page_number)) =
        parse_aligned_toc_continuation(line)
      {
        reasons[idx] = Some(NarrationSkipReason::Toc);
        if page_number.is_some() {
          pending_toc_row = false;
        }
        in_aligned_toc = true;
        continue;
      }
      pending_toc_row = false;
    }

    if let Some(toc_row) = parse_aligned_toc_row_start(line) {
      reasons[idx] = Some(NarrationSkipReason::Toc);
      pending_toc_row = toc_row.page_number.is_none();
      in_aligned_toc = true;
      continue;
    }
    if in_aligned_toc && parse_plain_aligned_toc_row(line).is_some() {
      reasons[idx] = Some(NarrationSkipReason::Toc);
      continue;
    }
    if looks_like_toc_entry(trimmed) {
      reasons[idx] = Some(NarrationSkipReason::Toc);
      in_aligned_toc = true;
      continue;
    }
    in_aligned_toc = false;

    if let Some(session_indent) = shell_session_indent.as_deref() {
      if shell_session_accepts(line, session_indent) {
        reasons[idx] = Some(NarrationSkipReason::Code);
        if looks_like_command_prompt_line(line) {
          shell_session_indent = Some(leading_whitespace(line).to_string());
        }
        continue;
      }
      shell_session_indent = None;
    }

    if looks_like_code_block_line(line) {
      reasons[idx] = Some(NarrationSkipReason::Code);
      table_context = None;
      code_continuation_indent_width = if code_line_continues(trimmed) {
        Some(char_len(leading_whitespace(line)))
      } else {
        None
      };
      if looks_like_command_prompt_line(line) {
        shell_session_indent = Some(leading_whitespace(line).to_string());
      }
      continue;
    }

    if let Some((indent, marker)) = table_context.as_ref() {
      if is_list_continuation_line(line, indent, marker) {
        reasons[idx] = Some(NarrationSkipReason::Table);
        continue;
      }
      table_context = None;
    }

    if let Some((indent, marker, _content, kind)) =
      parse_list_marker_with_kind(line)
      && matches!(
        kind,
        ListMarkerKind::OptionFlag | ListMarkerKind::FormatSpecifier
      )
    {
      reasons[idx] = Some(NarrationSkipReason::Table);
      table_context = Some((indent, marker));
    }
  }

  apply_markdown_table_reasons(lines, &mut reasons);
  apply_table_context_reasons(lines, &mut reasons);
  reasons
}

fn shell_session_accepts(line: &str, session_indent: &str) -> bool {
  if line.trim().is_empty() {
    return false;
  }

  let line_indent = leading_whitespace(line);
  let session_indent_width = char_len(session_indent);
  let line_indent_width = char_len(line_indent);
  if line_indent_width < session_indent_width {
    return false;
  }

  let extra_indent = line_indent_width - session_indent_width;
  line_indent == session_indent || extra_indent <= 12
}

fn is_fence_line(trimmed: &str) -> bool {
  trimmed.starts_with("```") || trimmed.starts_with("~~~")
}

fn table_cells(line: &str) -> Vec<&str> {
  line
    .trim()
    .trim_matches('|')
    .split('|')
    .map(str::trim)
    .filter(|cell| !cell.is_empty())
    .collect()
}

fn is_markdown_table_row(line: &str) -> bool {
  line.contains('|') && table_cells(line).len() >= 2
}

fn is_markdown_table_separator(line: &str) -> bool {
  if !line.contains('|') {
    return false;
  }
  let cells = table_cells(line);
  cells.len() >= 2
    && cells.iter().all(|cell| {
      cell.contains('-')
        && cell
          .chars()
          .all(|ch| matches!(ch, '-' | ':' | ' ') || ch.is_whitespace())
    })
}

fn apply_markdown_table_reasons(
  lines: &[String],
  reasons: &mut [Option<NarrationSkipReason>],
) {
  let mut idx = 0usize;
  while idx + 1 < lines.len() {
    if is_markdown_table_row(&lines[idx])
      && is_markdown_table_separator(&lines[idx + 1])
    {
      reasons[idx] = Some(NarrationSkipReason::Table);
      reasons[idx + 1] = Some(NarrationSkipReason::Table);
      idx += 2;
      while idx < lines.len() && is_markdown_table_row(&lines[idx]) {
        reasons[idx] = Some(NarrationSkipReason::Table);
        idx += 1;
      }
      continue;
    }
    idx += 1;
  }
}

fn apply_table_context_reasons(
  lines: &[String],
  reasons: &mut [Option<NarrationSkipReason>],
) {
  for idx in 0..lines.len() {
    if reasons[idx] != Some(NarrationSkipReason::Table) {
      continue;
    }

    let mut prev = idx;
    while prev > 0 {
      prev -= 1;
      let trimmed = lines[prev].trim();
      if trimmed.is_empty() || reasons[prev].is_some() {
        break;
      }
      if looks_like_table_or_figure_caption(trimmed)
        || looks_like_table_header(trimmed)
      {
        reasons[prev] = Some(NarrationSkipReason::Table);
        continue;
      }
      break;
    }
  }
}

fn looks_like_table_header(trimmed: &str) -> bool {
  let words: Vec<&str> = trimmed.split_whitespace().collect();
  words.len() >= 2
    && words.len() <= 5
    && words
      .last()
      .is_some_and(|last| matches!(*last, "Description" | "Notes" | "Grade"))
    && !trimmed.ends_with(['.', '!', '?'])
}
