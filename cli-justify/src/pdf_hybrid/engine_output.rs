use crate::text_utils::{char_len, leading_whitespace};
use crate::wrap::wrap_line_preserving_whitespace;

use super::code_blocks::{
  ensure_code_block_padding_above,
  ensure_code_block_padding_below_if_needed_and_reset,
  reindent_code_block_line,
};
use super::engine::FormatterEngine;
use super::structure::{
  looks_like_code_block_line, looks_like_command_prompt_line,
  looks_like_toc_entry, normalize_preserved_compact_layout_line,
  should_keep_pdf_line_layout,
};
use super::wrapping::flush_pending_pdf_block;

fn apply_deep_callout_bottom_margin(
  out: &mut Vec<String>,
  pending_margin: &mut bool,
) {
  if !*pending_margin {
    return;
  }
  if out.last().is_some_and(|last| !last.is_empty()) {
    out.push(String::new());
  }
  *pending_margin = false;
}

impl FormatterEngine {
  pub(super) fn close_code_block_and_clear_parent_indent(&mut self) {
    self.close_code_block_padding_and_reset();
    self.pending_code_block_parent_callout_indent = None;
  }

  pub(super) fn begin_preserved_layout_scope(&mut self) {
    self.in_aligned_toc = false;
    if let Some(capped_indent) = self.flush_pending_block_with_margin() {
      self.pending_code_block_parent_callout_indent = Some(capped_indent);
    }
    self.apply_pending_deep_callout_bottom_margin();
  }

  pub(super) fn handle_shell_session_line(&mut self, line: &str) -> bool {
    let Some(session_indent) = self.shell_session_indent.as_deref() else {
      return false;
    };
    if line.trim().is_empty() {
      return false;
    }

    let line_indent = leading_whitespace(line);
    let session_indent_width = char_len(session_indent);
    let line_indent_width = char_len(line_indent);

    if line_indent_width < session_indent_width {
      self.shell_session_indent = None;
      return false;
    }

    let extra_indent = line_indent_width - session_indent_width;
    let preserve = line_indent == session_indent || extra_indent <= 12;
    if !preserve {
      self.shell_session_indent = None;
      return false;
    }

    self.begin_preserved_layout_scope();
    self.emit_preserved_layout_line(line, true);
    true
  }

  pub(super) fn handle_preserved_pdf_layout_line(
    &mut self,
    line: &str,
  ) -> bool {
    if !should_keep_pdf_line_layout(line) {
      return false;
    }

    self.begin_preserved_layout_scope();

    let is_code_line = looks_like_code_block_line(line);
    if !is_code_line {
      self.close_code_block_and_clear_parent_indent();
    }

    self.emit_preserved_layout_line(line, is_code_line);
    true
  }

  pub(super) fn close_code_block_padding_and_reset(&mut self) {
    ensure_code_block_padding_below_if_needed_and_reset(
      &mut self.out,
      &mut self.in_code_block,
      &mut self.code_block_source_base_indent,
      &mut self.code_block_target_base_indent,
    );
  }

  pub(super) fn flush_pending_block_with_margin(&mut self) -> Option<usize> {
    let capped_indent = flush_pending_pdf_block(
      &mut self.pending,
      &mut self.out,
      self.line_width,
    );
    if capped_indent.is_some() {
      self.pending_deep_callout_bottom_margin = true;
    }
    capped_indent
  }

  pub(super) fn apply_pending_deep_callout_bottom_margin(&mut self) {
    apply_deep_callout_bottom_margin(
      &mut self.out,
      &mut self.pending_deep_callout_bottom_margin,
    );
  }

  pub(super) fn emit_preserved_layout_line(
    &mut self,
    line: &str,
    is_code_line: bool,
  ) {
    if is_code_line {
      ensure_code_block_padding_above(&mut self.out, &mut self.in_code_block);
    }

    let normalized_line = normalize_preserved_compact_layout_line(line);
    let normalized_line = if is_code_line {
      reindent_code_block_line(
        &normalized_line,
        &mut self.code_block_source_base_indent,
        &mut self.code_block_target_base_indent,
        &mut self.pending_code_block_parent_callout_indent,
      )
    } else {
      normalized_line
    };

    let starts_shell_prompt = looks_like_command_prompt_line(&normalized_line);
    if looks_like_toc_entry(normalized_line.trim()) {
      self.out.push(normalized_line);
    } else {
      self.out.extend(wrap_line_preserving_whitespace(
        &normalized_line,
        self.line_width,
      ));
    }

    if starts_shell_prompt {
      self.shell_session_indent = Some(leading_whitespace(line).to_string());
    }
  }
}
