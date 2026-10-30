use super::super::super::core::{Editor, EditorMode};
use crate::editor::char_boundary::CursorCol;

// Every kind of wide, zero-width or multi-char grapheme, and the display
// column each starts at: a 0, e+accent 1, ที่ 2, क+ि 3 (the vowel sign
// takes a cell), 中 5, Ａ 7, 👩‍💻 9, 🇪🇸 11, ​(zero-width space)+z 13.
const LINE: &str = "ae\u{301}ที่कि中Ａ👩\u{200d}💻🇪🇸\u{200b}z";

fn editor() -> Editor {
  let mut editor = Editor::new(vec![LINE.to_string()], 80);
  editor.offset = 0;
  editor.cursor_y = 0;
  editor
}

#[test]
fn cursor_screen_column_is_the_display_width_before_the_cursor() {
  let mut editor = editor();
  for (needle, column) in [
    ("a", 0),
    ("e", 1),
    ("ท", 2),
    ("क", 3),
    ("中", 5),
    ("Ａ", 7),
    ("👩", 9),
    ("🇪", 11),
    ("\u{200b}", 13),
    ("z", 13),
  ] {
    editor.cursor_x = CursorCol::lit(LINE.find(needle).unwrap());
    assert_eq!(editor.cursor_screen_column(), column, "{needle:?}");
  }
  // A byte inside a cluster reports the cluster's column.
  editor.cursor_x = CursorCol::lit(LINE.find('\u{301}').unwrap());
  assert_eq!(editor.cursor_screen_column(), 1);
  editor.cursor_x = CursorCol::lit(LINE.find('💻').unwrap());
  assert_eq!(editor.cursor_screen_column(), 9);
}

#[test]
fn terminal_cursor_is_moved_to_the_display_column() {
  let mut editor = editor();
  editor.show_cursor = true;
  editor.set_active_mode(EditorMode::Normal);
  editor.cursor_x = CursorCol::lit(LINE.find('👩').unwrap());
  let mut buffer = Vec::new();
  editor.position_cursor_buffered(&mut buffer, 4).unwrap();
  // MoveTo(4 + 9, 0) is "ESC [ 1 ; 14 H".
  let out = String::from_utf8(buffer).unwrap();
  assert!(out.contains("\u{1b}[1;14H"), "{out:?}");
}
