use crate::text_utils::{display_width, leading_whitespace};

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
  reset_code_block_indent_state(source_base_indent, target_base_indent);
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
  let line_indent_width = display_width(line_indent);

  if source_base_indent.is_none() || target_base_indent.is_none() {
    *source_base_indent = Some(line_indent_width);
    let target_base = pending_parent_callout_indent
      .map(|indent| indent + DEEP_CALLOUT_CODE_BLOCK_EXTRA_INDENT_CHARS)
      .unwrap_or(DEFAULT_CODE_BLOCK_BASE_INDENT_CHARS);
    *target_base_indent = Some(target_base);
    *pending_parent_callout_indent = None;
  }

  let source_base = source_base_indent.expect("source indent initialized");
  if line_indent_width < source_base {
    *source_base_indent = Some(line_indent_width);
  }
  let source_base = source_base_indent.expect("source indent initialized");
  let target_base = target_base_indent.expect("target indent initialized");
  let relative_indent = if looks_like_command_prompt_line(trimmed) {
    0
  } else {
    line_indent_width.saturating_sub(source_base)
  };
  let new_indent = target_base + relative_indent;
  format!("{}{}", " ".repeat(new_indent), trimmed)
}

#[cfg(test)]
mod tests {
  use super::{
    ensure_code_block_padding_above, ensure_code_block_padding_below_if_needed,
    ensure_code_block_padding_below_if_needed_and_reset,
    reindent_code_block_line,
  };

  #[test]
  fn code_block_padding_is_added_only_at_transitions() {
    let mut out = Vec::new();
    let mut in_code_block = false;
    ensure_code_block_padding_above(&mut out, &mut in_code_block);
    assert!(out.is_empty());
    assert!(in_code_block);

    out.push("  code".to_string());
    ensure_code_block_padding_above(&mut out, &mut in_code_block);
    assert_eq!(out, ["  code"]);
    ensure_code_block_padding_below_if_needed(&mut out, &mut in_code_block);
    assert_eq!(out, ["  code", ""]);
    assert!(!in_code_block);

    in_code_block = true;
    ensure_code_block_padding_below_if_needed(&mut out, &mut in_code_block);
    assert_eq!(out, ["  code", ""]);
    assert!(!in_code_block);

    ensure_code_block_padding_below_if_needed(&mut out, &mut in_code_block);
    assert_eq!(out, ["  code", ""]);

    out.push("following prose".to_string());
    ensure_code_block_padding_above(&mut out, &mut in_code_block);
    assert_eq!(out.last().map(String::as_str), Some(""));
  }

  #[test]
  fn resetting_a_code_block_clears_its_indent_state() {
    let mut out = vec!["  code".to_string()];
    let mut in_code_block = true;
    let mut source = Some(4);
    let mut target = Some(2);
    ensure_code_block_padding_below_if_needed_and_reset(
      &mut out,
      &mut in_code_block,
      &mut source,
      &mut target,
    );
    assert_eq!(out.last().map(String::as_str), Some(""));
    assert!(!in_code_block);
    assert_eq!((source, target), (None, None));
  }

  #[test]
  fn reindent_preserves_relative_depth_and_handles_tabs() {
    let mut source = None;
    let mut target = None;
    let mut parent = None;
    assert_eq!(
      reindent_code_block_line("   ", &mut source, &mut target, &mut parent),
      "   "
    );
    assert_eq!(
      reindent_code_block_line(
        "    one",
        &mut source,
        &mut target,
        &mut parent
      ),
      "  one"
    );
    assert_eq!(
      reindent_code_block_line(
        "      two",
        &mut source,
        &mut target,
        &mut parent
      ),
      "    two"
    );
    assert_eq!(
      reindent_code_block_line(
        "  three",
        &mut source,
        &mut target,
        &mut parent
      ),
      "  three"
    );
    assert_eq!(
      reindent_code_block_line("\tfour", &mut source, &mut target, &mut parent),
      "        four"
    );
    assert_eq!((source, target, parent), (Some(2), Some(2), None));
  }

  #[test]
  fn parent_callout_sets_the_code_indent() {
    let mut source = None;
    let mut target = None;
    let mut parent = Some(4);
    assert_eq!(
      reindent_code_block_line(
        "    code",
        &mut source,
        &mut target,
        &mut parent
      ),
      "      code"
    );
    assert_eq!((source, target, parent), (Some(4), Some(6), None));

    target = None;
    assert_eq!(
      reindent_code_block_line(
        "    following",
        &mut source,
        &mut target,
        &mut parent
      ),
      "  following"
    );
    assert_eq!((source, target), (Some(4), Some(2)));
  }
}
