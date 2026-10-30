use super::*;
use crate::editor::char_boundary::CursorCol;

fn new_editor(lines: &[&str]) -> Editor {
  let mut editor =
    Editor::new(lines.iter().map(|l| l.to_string()).collect(), 80);
  editor.height = 10;
  editor.offset = 0;
  editor.cursor_y = 0;
  editor
}

#[test]
fn find_prev_line_end_returns_byte_offset_of_last_word() {
  let editor =
    new_editor(&["hola canción", "precio 5€€", "日本語", "fin…  ", "abc def"]);
  assert_eq!(editor.find_prev_line_end(0).to_raw(), (0, 5));
  // The trailing punctuation run "€€" starts at byte 8.
  assert_eq!(editor.find_prev_line_end(1).to_raw(), (1, 8));
  assert_eq!(editor.find_prev_line_end(2).to_raw(), (2, 0));
  // '…' (3 bytes) starts at byte 3; trailing spaces are skipped.
  assert_eq!(editor.find_prev_line_end(3).to_raw(), (3, 3));
  assert_eq!(editor.find_prev_line_end(4).to_raw(), (4, 4));
}

#[test]
fn find_sentence_boundary_uses_byte_offsets() {
  // ¿0..2 Q2 u3 é4..6 ' '6 t7 a8 l9 ?10 ' '11 B12 i13 e14 n15 .16 ' '17
  // A18 d19 i20 ó21..23 s23
  let mut editor = new_editor(&["¿Qué tal? Bien. Adiós"]);
  editor.cursor_x = CursorCol::lit(0);
  assert_eq!(editor.find_sentence_boundary(true).to_raw(), (0, 12));
  editor.cursor_x = CursorCol::lit(12);
  assert_eq!(editor.find_sentence_boundary(true).to_raw(), (0, 18));
  editor.cursor_x = CursorCol::lit(18);
  assert_eq!(editor.find_sentence_boundary(false).to_raw(), (0, 17));
  // No further sentence: go to the start of the last character.
  editor.cursor_x = CursorCol::lit(21);
  assert_eq!(editor.find_sentence_boundary(true).to_raw(), (0, 23));
}

#[test]
fn find_prev_line_end_lands_on_cluster_starts() {
  let lines = ["hola ที่นี่", "x 👩\u{200d}💻🇪🇸", "cafe\u{301}"];
  let editor = new_editor(&lines);
  assert_eq!(
    editor.find_prev_line_end(0).to_raw(),
    (0, lines[0].find("ที่").unwrap())
  );
  assert_eq!(editor.find_prev_line_end(1).to_raw(), (1, 2));
  assert_eq!(editor.find_prev_line_end(2).to_raw(), (2, 0));
}

#[test]
fn find_sentence_boundary_lands_on_cluster_starts() {
  let line = "C\u{327}a va. 中文 ok! e\u{301}l";
  let mut editor = new_editor(&[line]);
  editor.cursor_x = CursorCol::lit(0);
  assert_eq!(
    editor.find_sentence_boundary(true).to_raw(),
    (0, line.find('中').unwrap())
  );
  editor.cursor_x = CursorCol::lit(line.find('中').unwrap());
  assert_eq!(
    editor.find_sentence_boundary(true).to_raw(),
    (0, line.find("e\u{301}").unwrap())
  );
  // No further sentence: the last cluster.
  editor.cursor_x = CursorCol::lit(line.find('l').unwrap());
  assert_eq!(
    editor.find_sentence_boundary(true).to_raw(),
    (0, line.find('l').unwrap())
  );
}
