use super::{FormatterEngine, apply_deep_callout_bottom_margin};
use crate::pdf_hybrid::engine::PendingPdfBlock;

#[test]
fn pending_callout_margin_is_applied_once_after_content() {
  let mut out = Vec::new();
  let mut pending = false;
  apply_deep_callout_bottom_margin(&mut out, &mut pending);
  assert!(out.is_empty());

  pending = true;
  out.push("callout".into());
  apply_deep_callout_bottom_margin(&mut out, &mut pending);
  assert_eq!(out, ["callout", ""]);
  assert!(!pending);

  pending = true;
  apply_deep_callout_bottom_margin(&mut out, &mut pending);
  assert_eq!(out, ["callout", ""]);
  assert!(!pending);
}

#[test]
fn shell_session_scope_ends_on_blank_or_distant_lines() {
  let mut engine = FormatterEngine::new(40);
  assert!(!engine.handle_shell_session_line("  output"));
  engine.shell_session_indent = Some("  ".into());
  assert!(!engine.handle_shell_session_line(""));
  assert_eq!(engine.shell_session_indent.as_deref(), Some("  "));
  assert!(engine.handle_shell_session_line("  ordinary output"));
  assert!(engine.handle_shell_session_line("    more output"));
  assert!(!engine.handle_shell_session_line("output at root"));
  assert!(engine.shell_session_indent.is_none());

  engine.shell_session_indent = Some("  ".into());
  assert!(
    !engine.handle_shell_session_line(&format!("{}too far", " ".repeat(16)))
  );
  assert!(engine.shell_session_indent.is_none());
}

#[test]
fn code_continuations_keep_the_scope_only_while_the_shape_matches() {
  let mut engine = FormatterEngine::new(40);
  assert!(!engine.handle_code_continuation_line("  && next"));
  engine.code_continuation_indent_width = Some(2);
  assert!(engine.handle_code_continuation_line("  && next"));
  engine.code_continuation_indent_width = Some(2);
  assert!(!engine.handle_code_continuation_line("ordinary prose"));
  assert!(engine.code_continuation_indent_width.is_none());
}

#[test]
fn preserved_layout_closes_code_blocks_before_regular_rows() {
  let mut engine = FormatterEngine::new(40);
  assert!(!engine.handle_preserved_pdf_layout_line("ordinary prose"));
  engine.emit_preserved_layout_line("    $ git status |", true);
  assert!(engine.in_code_block);
  assert!(engine.code_continuation_indent_width.is_some());

  assert!(engine.handle_preserved_pdf_layout_line("\t中文 title"));
  assert!(!engine.in_code_block);
  assert!(engine.code_continuation_indent_width.is_none());
  assert!(engine.out.iter().any(|line| line.contains("中文 title")));

  engine.emit_preserved_layout_line("Chapter 1 .... 中文 title .... 12", false);
  assert_eq!(
    engine.out.last().map(String::as_str),
    Some("Chapter 1 .... 中文 title .... 12")
  );
}

#[test]
fn flushing_a_deep_paragraph_sets_the_parent_indent_and_margin() {
  let mut engine = FormatterEngine::new(40);
  engine.pending = Some(PendingPdfBlock::Paragraph {
    indent: "             ".into(),
    lines: vec!["one two three four five six seven".into()],
  });
  engine.begin_preserved_layout_scope();
  assert_eq!(engine.pending_code_block_parent_callout_indent, Some(12));
  assert!(!engine.pending_deep_callout_bottom_margin);
  assert_eq!(engine.out.last().map(String::as_str), Some(""));

  engine.in_code_block = true;
  engine.code_continuation_indent_width = Some(4);
  engine.close_code_block_and_clear_parent_indent();
  assert!(!engine.in_code_block);
  assert!(engine.pending_code_block_parent_callout_indent.is_none());
  assert!(engine.code_continuation_indent_width.is_none());
}
