// Issue #99 end to end: where the terminal cursor is placed on accented text,
// and `v` then `l` across it with a redraw after every key. Shares the
// fixtures of `utf8_tests`.

use crossterm::QueueableCommand;
use crossterm::cursor::MoveTo;

use super::super::core::{Editor, EditorMode};
use super::utf8_tests::{LINE, editor, key, utf8};
use crate::editor::char_boundary::CursorCol;

// The terminal cursor is placed by character column, not byte offset, so it
// sits on the character the highlight is drawn under.
#[test]
fn cursor_is_positioned_by_character_column() {
  let mut editor = editor();
  editor.show_cursor = true;
  // The character after the accented one in "canción": its byte offset and
  // its character column differ.
  let byte = LINE.find("ción").unwrap() + "ció".len();
  editor.cursor_x = CursorCol::lit(byte);
  let column = LINE[..byte].chars().count();
  assert_ne!(column, byte);

  let mut buffer = Vec::new();
  editor.position_cursor_buffered(&mut buffer, 2).unwrap();
  let mut want = Vec::new();
  want.queue(MoveTo((2 + column) as u16, 0)).unwrap();
  let out = utf8(buffer);
  assert!(out.contains(&utf8(want)), "got {out:?}");

  // Past the last line there is no text to measure; fall back to the byte
  // offset rather than panicking.
  editor.cursor_y = 4;
  editor.cursor_x = CursorCol::lit(3);
  let mut buffer = Vec::new();
  editor.position_cursor_buffered(&mut buffer, 0).unwrap();
  let mut want = Vec::new();
  want.queue(MoveTo(3, 4)).unwrap();
  assert!(utf8(buffer).contains(&utf8(want)));
}

// End-to-end reproduction of issue #99: `v` then `l` across accented text,
// redrawing after every keypress.
fn press_l_across_line(editor: &mut Editor, line: &str) {
  editor.handle_normal_mode_event(key('v')).unwrap();
  assert_eq!(editor.get_active_mode(), EditorMode::VisualChar);
  for _ in 0..line.len() + 2 {
    editor.handle_visual_mode_event(key('l')).unwrap();
    let mut buffer = Vec::new();
    editor.draw_content_buffered(&mut buffer, 80, "").unwrap();
    utf8(buffer);
  }
}

#[test]
fn issue_99_visual_l_over_accents_does_not_panic() {
  let line = "La canción está aquí";
  let mut editor = Editor::new(vec![line.into()], 80);
  editor.height = 5;
  editor.offset = 0;
  editor.cursor_y = 0;
  press_l_across_line(&mut editor, line);
}

#[test]
fn issue_99_visual_l_over_accents_with_persistent_highlight() {
  let line = "La canción está aquí";
  let mut editor = Editor::new(vec![line.into()], 80);
  editor.height = 5;
  editor.offset = 0;
  editor.cursor_y = 0;
  // "está", so the selection grows into and across the persistent highlight
  // and the combined renderer draws the line.
  let start = line.find("está").unwrap();
  editor.highlights.add_highlight(start, start + "está".len());
  assert!(editor.has_persistent_highlights_on_line(0));
  press_l_across_line(&mut editor, line);
}
