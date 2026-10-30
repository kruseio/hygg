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

// Selects with `select` at byte `col` and returns the selected text, after
// checking the selection columns are on boundaries.
fn selected(line: &str, col: usize, select: fn(&mut Editor)) -> String {
  let mut editor = new_editor(&[line]);
  editor.cursor_x = CursorCol::lit(col);
  select(&mut editor);
  let (_, start) = editor.editor_state.selection_start.unwrap().to_raw();
  let (_, end) = editor.editor_state.selection_end.unwrap().to_raw();
  assert!(line.is_char_boundary(start) && line.is_char_boundary(end));
  line[start..end].to_string()
}

fn iw(e: &mut Editor) {
  e.select_inner_word(false);
}
fn i_big_w(e: &mut Editor) {
  e.select_inner_word(true);
}
fn aw(e: &mut Editor) {
  e.select_around_word(false);
}
fn a_big_w(e: &mut Editor) {
  e.select_around_word(true);
}

#[test]
fn inner_word_on_accented_words() {
  let mut editor = new_editor(&[LINE]);
  editor.cursor_x = CursorCol::lit(8);
  editor.select_inner_word(false);
  assert_eq!(
    editor.editor_state.selection_start.map(Pos::to_raw),
    Some((0, 3))
  );
  assert_eq!(editor.editor_state.selection_end.map(Pos::to_raw), Some((0, 11)));

  assert_eq!(selected(LINE, 15, iw), "está");
  assert_eq!(selected(LINE, 21, iw), "aquí");
  // Past the end clamps onto the last character.
  assert_eq!(selected(LINE, 99, iw), "aquí");
  assert_eq!(selected("日本語 text", 3, iw), "日本語");
}

#[test]
fn inner_word_and_big_word_with_three_byte_punctuation() {
  // "precio: 5€ total": 5 at byte 8, € 9..12.
  let line = "precio: 5€ total";
  assert_eq!(selected(line, 9, iw), "€");
  assert_eq!(selected(line, 8, iw), "5");
  assert_eq!(selected(line, 9, i_big_w), "5€");
  assert_eq!(selected(line, 13, i_big_w), "total");
}

#[test]
fn around_word_on_accented_words() {
  let mut editor = new_editor(&[LINE]);
  editor.cursor_x = CursorCol::lit(8);
  editor.select_around_word(false);
  // Existing behaviour: with a word after it, both the trailing and the
  // leading whitespace are taken.
  assert_eq!(
    editor.editor_state.selection_start.map(Pos::to_raw),
    Some((0, 2))
  );
  assert_eq!(editor.editor_state.selection_end.map(Pos::to_raw), Some((0, 12)));

  assert_eq!(selected(LINE, 15, aw), " está ");
  // Last word: nothing after it and no leading whitespace either.
  assert_eq!(selected(LINE, 21, aw), "aquí");
  assert_eq!(selected("precio: 5€ total", 9, a_big_w), " 5€ ");
  assert_eq!(selected("日本語 text", 3, aw), "日本語 ");
}

#[test]
fn ascii_word_selection_is_unchanged() {
  assert_eq!(selected("foo.bar baz", 5, iw), "bar");
  assert_eq!(selected("foo.bar baz", 5, i_big_w), "foo.bar");
  assert_eq!(selected("foo.bar baz", 5, aw), "bar ");
  assert_eq!(selected("foo.bar baz", 9, aw), "baz");
  assert_eq!(selected("one two three", 5, aw), " two ");
}

#[test]
fn word_selection_takes_whole_clusters() {
  let line = "cafe\u{301} x";
  assert_eq!(selected(line, 0, iw), "cafe\u{301}");
  assert_eq!(selected(line, 4, iw), "cafe\u{301}");
  assert_eq!(selected(line, 0, aw), "cafe\u{301} ");
  let line = "x👩\u{200d}💻🇪🇸 ที่นี่";
  assert_eq!(selected(line, 1, i_big_w), "x👩\u{200d}💻🇪🇸");
  assert_eq!(selected(line, 9, iw), "👩\u{200d}💻🇪🇸");
  assert_eq!(selected(line, line.find("นี่").unwrap() + 3, iw), "ที่นี่");
  // Last word on the line: no trailing and (existing behaviour) no leading
  // whitespace.
  assert_eq!(selected(line, line.find("ที่").unwrap(), a_big_w), "ที่นี่");
  assert_eq!(selected(line, 1, a_big_w), "x👩\u{200d}💻🇪🇸 ");
  assert_eq!(selected("Ａｂｃ 中文", 3, iw), "Ａｂｃ");
}
