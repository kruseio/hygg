use crossterm::cursor::SetCursorStyle;

use super::super::core::{Editor, EditorMode};
use crate::editor::streaming::{PendingPdfStream, StreamReady};
use std::sync::mpsc;
use std::time::Instant;

fn test_editor() -> Editor {
  let mut editor = Editor::new(vec!["line".to_string()], 80);
  editor.height = 24;
  editor.width = 80;
  editor.cursor_x = 0;
  editor.cursor_y = 0;
  editor
}

fn rendered(buffer: Vec<u8>) -> String {
  String::from_utf8(buffer).expect("cursor commands should be utf8")
}

#[test]
fn buffered_cursor_hides_when_show_cursor_is_false() {
  let mut editor = test_editor();
  editor.cursor_currently_visible = true;
  editor.show_cursor = false;

  let mut buffer = Vec::new();
  editor.position_cursor_buffered(&mut buffer, 0).unwrap();

  assert!(rendered(buffer).contains("\x1b[?25l"));
  assert!(!editor.cursor_currently_visible);
}

#[test]
fn buffered_cursor_hides_while_pdf_is_pending() {
  let mut editor = test_editor();
  editor.cursor_currently_visible = true;
  let (_tx, rx) = mpsc::channel::<StreamReady>();
  editor.pdf_pending = Some(PendingPdfStream {
    receiver: rx,
    started_at: Instant::now(),
    canonical_path_display: "pending.pdf".to_string(),
    restore_line_in_page: None,
    restore_cursor_y: None,
  });

  let mut buffer = Vec::new();
  editor.position_cursor_buffered(&mut buffer, 0).unwrap();

  assert!(rendered(buffer).contains("\x1b[?25l"));
  assert!(!editor.cursor_currently_visible);
}

#[test]
fn buffered_cursor_reuses_visible_state_and_cached_style() {
  let mut editor = test_editor();
  editor.cursor_currently_visible = true;
  editor.last_cursor_style = Some(SetCursorStyle::BlinkingBlock);
  editor.set_active_mode(EditorMode::Normal);

  let mut buffer = Vec::new();
  editor.position_cursor_buffered(&mut buffer, 0).unwrap();

  let output = rendered(buffer);
  assert!(!output.contains("\x1b[?25h"));
  assert!(!output.contains("\x1b[1 q"));
  assert!(editor.cursor_currently_visible);
  assert_eq!(editor.last_cursor_style, Some(SetCursorStyle::BlinkingBlock));
}

#[test]
fn buffered_cursor_queues_style_when_mode_changes() {
  let mut editor = test_editor();
  editor.cursor_currently_visible = true;
  editor.last_cursor_style = Some(SetCursorStyle::BlinkingBlock);
  editor.set_active_mode(EditorMode::VisualChar);

  let mut buffer = Vec::new();
  editor.position_cursor_buffered(&mut buffer, 0).unwrap();

  let output = rendered(buffer);
  assert!(output.contains("\x1b[2 q"));
  assert!(!output.contains("\x1b[?25h"));
  assert_eq!(editor.last_cursor_style, Some(SetCursorStyle::SteadyBlock));
}
