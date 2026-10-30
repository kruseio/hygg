use super::*;
use crate::editor::char_boundary::{CursorCol, Pos};

// "La canción está aquí": 'ó' is bytes 8..10, 'á' 15..17, 'í' 21..23.
const LINE: &str = "La canción está aquí";

fn new_editor(lines: &[&str]) -> Editor {
  let mut editor =
    Editor::new(lines.iter().map(|l| l.to_string()).collect(), 80);
  editor.height = 10;
  editor.offset = 0;
  editor.cursor_y = 0;
  editor
}

#[test]
fn f_and_t_forward_from_a_multibyte_char() {
  let mut editor = new_editor(&[LINE]);
  // Cursor on 'ó': `line[col + 1..]` used to slice inside it.
  editor.cursor_x = CursorCol::lit(8);
  assert_eq!(
    editor.find_char_on_line('a', true, false).map(CursorCol::raw),
    Some(18)
  );
  assert_eq!(
    editor.find_char_on_line('a', true, true).map(CursorCol::raw),
    Some(17)
  );
  // `t` onto the character right after 'ó' stops on the 'ó' itself, not
  // one byte before the target.
  editor.cursor_x = CursorCol::lit(6);
  assert_eq!(
    editor.find_char_on_line('n', true, true).map(CursorCol::raw),
    Some(8)
  );
  // Targets that are themselves multi-byte.
  editor.cursor_x = CursorCol::lit(0);
  assert_eq!(
    editor.find_char_on_line('á', true, false).map(CursorCol::raw),
    Some(15)
  );
  assert_eq!(
    editor.find_char_on_line('á', true, true).map(CursorCol::raw),
    Some(14)
  );
  // On the last character there is nothing left to search.
  editor.cursor_x = CursorCol::lit(21);
  assert_eq!(
    editor.find_char_on_line('í', true, false).map(CursorCol::raw),
    None
  );
}

#[test]
fn f_and_t_backward_land_on_boundaries() {
  let mut editor = new_editor(&[LINE]);
  editor.cursor_x = CursorCol::lit(8);
  assert_eq!(
    editor.find_char_on_line('c', false, false).map(CursorCol::raw),
    Some(6)
  );
  assert_eq!(
    editor.find_char_on_line('c', false, true).map(CursorCol::raw),
    Some(7)
  );
  // `T` past a multi-byte target stops on the next character, not one byte
  // into the target.
  editor.cursor_x = CursorCol::lit(12);
  assert_eq!(
    editor.find_char_on_line('ó', false, false).map(CursorCol::raw),
    Some(8)
  );
  assert_eq!(
    editor.find_char_on_line('ó', false, true).map(CursorCol::raw),
    Some(10)
  );
  editor.cursor_x = CursorCol::lit(18);
  assert_eq!(
    editor.find_char_on_line('á', false, true).map(CursorCol::raw),
    Some(17)
  );
}

#[test]
fn f_and_t_with_three_byte_chars() {
  // 5 0, € 1..4, ' ' 4, y 5, ' ' 6, 7 7, € 8..11
  let mut editor = new_editor(&["5€ y 7€"]);
  editor.cursor_x = CursorCol::lit(0);
  assert_eq!(
    editor.find_char_on_line('€', true, false).map(CursorCol::raw),
    Some(1)
  );
  assert_eq!(
    editor.find_char_on_line('€', true, true).map(CursorCol::raw),
    Some(0)
  );
  editor.cursor_x = CursorCol::lit(1);
  assert_eq!(
    editor.find_char_on_line('€', true, false).map(CursorCol::raw),
    Some(8)
  );
  assert_eq!(
    editor.find_char_on_line('€', true, true).map(CursorCol::raw),
    Some(7)
  );
  editor.cursor_x = CursorCol::lit(8);
  assert_eq!(
    editor.find_char_on_line('€', false, false).map(CursorCol::raw),
    Some(1)
  );
  assert_eq!(
    editor.find_char_on_line('€', false, true).map(CursorCol::raw),
    Some(4)
  );
  assert_eq!(
    editor.find_char_on_line('5', false, true).map(CursorCol::raw),
    Some(1)
  );
}

#[test]
fn find_text_object_returns_byte_offsets() {
  // ( is byte 5, € is 15..18, ) is byte 18.
  let line = "dijo (canción €) ya";
  let mut editor = new_editor(&[line]);
  for col in [6, 11, 15] {
    editor.cursor_x = CursorCol::lit(col);
    assert_eq!(
      editor.find_text_object('(').map(|(s, e)| (s.raw(), e.raw())),
      Some((6, 18))
    );
  }
  assert_eq!(&line[6..18], "canción €");
  editor.cursor_x = CursorCol::lit(20);
  assert_eq!(editor.find_text_object('('), None);
}

#[test]
fn find_matching_bracket_on_multibyte_line() {
  // [ 0, € 1..4, ' ' 4, ( 5, a 6, ñ 7..9, ) 9, ' ' 10, ] 11
  let mut editor = new_editor(&["[€ (añ) ]"]);
  for (col, expected) in [
    (0, Some((0, 11))),
    (11, Some((0, 0))),
    (5, Some((0, 9))),
    (9, Some((0, 5))),
  ] {
    editor.cursor_x = CursorCol::lit(col);
    assert_eq!(
      editor.find_matching_bracket().map(Pos::to_raw),
      expected,
      "from {col}"
    );
  }
  // On a multi-byte non-bracket: no match, no panic.
  editor.cursor_x = CursorCol::lit(7);
  assert_eq!(editor.find_matching_bracket().map(Pos::to_raw), None);
  editor.cursor_x = CursorCol::lit(2);
  assert_eq!(editor.find_matching_bracket().map(Pos::to_raw), None);
}

#[test]
fn find_matching_bracket_across_lines() {
  let mut editor = new_editor(&["ñ (ó", "é)"]);
  editor.cursor_x = CursorCol::lit(3);
  assert_eq!(editor.find_matching_bracket().map(Pos::to_raw), Some((1, 2)));
  editor.cursor_y = 1;
  editor.cursor_x = CursorCol::lit(2);
  assert_eq!(editor.find_matching_bracket().map(Pos::to_raw), Some((0, 3)));
}

// a 0, e 1, U+0301 2..4, x 4, 👩 5..9, ZWJ 9..12, 💻 12..16, y 16,
// 🇪 17..21, 🇸 21..25, z 25
const CLUSTERS: &str = "ae\u{301}x👩\u{200d}💻y🇪🇸z";

#[test]
fn f_and_t_land_on_grapheme_starts() {
  let mut editor = new_editor(&[CLUSTERS]);
  editor.cursor_x = CursorCol::lit(0);
  assert_eq!(
    editor.find_char_on_line('x', true, false).map(CursorCol::raw),
    Some(4)
  );
  // 't' stops on 'e', not on its combining accent.
  assert_eq!(
    editor.find_char_on_line('x', true, true).map(CursorCol::raw),
    Some(1)
  );
  // A target inside a cluster lands on the cluster.
  assert_eq!(
    editor.find_char_on_line('\u{301}', true, false).map(CursorCol::raw),
    Some(1)
  );
  editor.cursor_x = CursorCol::lit(4);
  assert_eq!(
    editor.find_char_on_line('💻', true, false).map(CursorCol::raw),
    Some(5)
  );
  assert_eq!(
    editor.find_char_on_line('y', true, true).map(CursorCol::raw),
    Some(5)
  );
  editor.cursor_x = CursorCol::lit(16);
  assert_eq!(
    editor.find_char_on_line('🇸', true, false).map(CursorCol::raw),
    Some(17)
  );
  // From the emoji, the search starts after the whole cluster.
  editor.cursor_x = CursorCol::lit(5);
  assert_eq!(
    editor.find_char_on_line('y', true, false).map(CursorCol::raw),
    Some(16)
  );
  assert_eq!(
    editor.find_char_on_line('💻', true, false).map(CursorCol::raw),
    None
  );
}

#[test]
fn backward_f_and_t_land_on_grapheme_starts() {
  let mut editor = new_editor(&[CLUSTERS]);
  editor.cursor_x = CursorCol::lit(25);
  assert_eq!(
    editor.find_char_on_line('e', false, false).map(CursorCol::raw),
    Some(1)
  );
  assert_eq!(
    editor.find_char_on_line('\u{301}', false, false).map(CursorCol::raw),
    Some(1)
  );
  assert_eq!(
    editor.find_char_on_line('x', false, true).map(CursorCol::raw),
    Some(5)
  );
  // 'T' past the emoji steps over all of it.
  assert_eq!(
    editor.find_char_on_line('💻', false, true).map(CursorCol::raw),
    Some(16)
  );
  assert_eq!(
    editor.find_char_on_line('👩', false, true).map(CursorCol::raw),
    Some(16)
  );
  // From inside the flag: as from its start.
  editor.cursor_x = CursorCol::lit(21);
  assert_eq!(
    editor.find_char_on_line('y', false, false).map(CursorCol::raw),
    Some(16)
  );
  assert_eq!(
    editor.find_char_on_line('🇪', false, false).map(CursorCol::raw),
    None
  );
}
