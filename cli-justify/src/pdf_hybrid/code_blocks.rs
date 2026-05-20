use crate::text_utils::{char_len, leading_whitespace};

use super::structure::looks_like_command_prompt_line;

const DEFAULT_CODE_BLOCK_BASE_INDENT_CHARS: usize = 2;
const DEEP_CALLOUT_CODE_BLOCK_EXTRA_INDENT_CHARS: usize = 2;

pub(super) fn ensure_code_block_padding_above(
  out: &mut Vec<String>,
  in_code_block: &mut bool,
) {
  if !*in_code_block && out.last().is_some_and(|last| !last.is_empty()) {
    out.push(String::new());
  }
  *in_code_block = true;
}

pub(super) fn ensure_code_block_padding_below_if_needed(
  out: &mut Vec<String>,
  in_code_block: &mut bool,
) {
  if !*in_code_block {
    return;
  }
  if out.last().is_some_and(|last| !last.is_empty()) {
    out.push(String::new());
  }
  *in_code_block = false;
}

pub(super) fn ensure_code_block_padding_below_if_needed_and_reset(
  out: &mut Vec<String>,
  in_code_block: &mut bool,
  source_base_indent: &mut Option<usize>,
  target_base_indent: &mut Option<usize>,
) {
  ensure_code_block_padding_below_if_needed(out, in_code_block);
  if !*in_code_block {
    reset_code_block_indent_state(source_base_indent, target_base_indent);
  }
}

pub(super) fn reset_code_block_indent_state(
  source_base_indent: &mut Option<usize>,
  target_base_indent: &mut Option<usize>,
) {
  *source_base_indent = None;
  *target_base_indent = None;
}

pub(super) fn reindent_code_block_line(
  line: &str,
  source_base_indent: &mut Option<usize>,
  target_base_indent: &mut Option<usize>,
  pending_parent_callout_indent: &mut Option<usize>,
) -> String {
  let trimmed = line.trim_start_matches([' ', '\t']);
  if trimmed.is_empty() {
    return line.to_string();
  }

  let line_indent = leading_whitespace(line);
  let line_indent_width = char_len(line_indent);

  if source_base_indent.is_none() || target_base_indent.is_none() {
    *source_base_indent = Some(line_indent_width);
    let target_base = pending_parent_callout_indent
      .map(|indent| indent + DEEP_CALLOUT_CODE_BLOCK_EXTRA_INDENT_CHARS)
      .unwrap_or(DEFAULT_CODE_BLOCK_BASE_INDENT_CHARS);
    *target_base_indent = Some(target_base);
    *pending_parent_callout_indent = None;
  }

  let source_base = source_base_indent.unwrap_or(line_indent_width);
  if line_indent_width < source_base {
    *source_base_indent = Some(line_indent_width);
  }
  let source_base = source_base_indent.unwrap_or(line_indent_width);
  let target_base =
    target_base_indent.unwrap_or(DEFAULT_CODE_BLOCK_BASE_INDENT_CHARS);
  let relative_indent = if looks_like_command_prompt_line(trimmed) {
    0
  } else {
    line_indent_width.saturating_sub(source_base)
  };
  let new_indent = target_base + relative_indent;
  format!("{}{}", " ".repeat(new_indent), trimmed)
}
