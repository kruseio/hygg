use super::*;
use crate::editor::char_boundary::CursorCol;

// "La canción está aquí": 'ó' is bytes 8..10, 'á' 15..17, 'í' 21..23.
// Word starts are at bytes 0, 3, 12, 18.
const LINE: &str = "La canción está aquí";

fn new_editor(lines: &[&str]) -> Editor {
  let mut editor =
    Editor::new(lines.iter().map(|l| l.to_string()).collect(), 80);
  editor.height = 10;
  editor.offset = 0;
  editor.cursor_y = 0;
  editor
}

fn word_forward(editor: &mut Editor, col: usize, big: bool) -> usize {
  editor.cursor_x = CursorCol::lit(col);
  editor.find_word_boundary(true, big).col.raw()
}

fn word_backward(editor: &mut Editor, col: usize, big: bool) -> usize {
  editor.cursor_x = CursorCol::lit(col);
  editor.find_word_boundary(false, big).col.raw()
}

fn word_end(editor: &mut Editor, col: usize, big: bool) -> usize {
  editor.cursor_x = CursorCol::lit(col);
  editor.find_word_end(big).col.raw()
}

#[test]
fn w_and_b_land_on_byte_offsets_of_accented_words() {
  let mut editor = new_editor(&[LINE]);
  assert_eq!(word_forward(&mut editor, 0, false), 3);
  assert_eq!(word_forward(&mut editor, 3, false), 12);
  assert_eq!(word_forward(&mut editor, 8, false), 12);
  assert_eq!(word_forward(&mut editor, 12, false), 18);
  assert_eq!(word_backward(&mut editor, 18, false), 12);
  assert_eq!(word_backward(&mut editor, 12, false), 3);
  assert_eq!(word_backward(&mut editor, 15, false), 12);
  assert_eq!(word_backward(&mut editor, 3, false), 0);
}

#[test]
fn w_and_b_step_over_three_byte_punctuation() {
  // '€' is bytes 9..12 and not a word character.
  let mut editor = new_editor(&["precio: 5€ total"]);
  assert_eq!(word_forward(&mut editor, 8, false), 9);
  assert_eq!(word_forward(&mut editor, 9, false), 13);
  assert_eq!(word_forward(&mut editor, 8, true), 13);
  assert_eq!(word_backward(&mut editor, 13, false), 9);
  assert_eq!(word_backward(&mut editor, 13, true), 8);
}

#[test]
fn w_and_e_on_cjk_words() {
  let mut editor = new_editor(&["日本語 text"]);
  assert_eq!(word_forward(&mut editor, 0, false), 10);
  assert_eq!(word_end(&mut editor, 0, false), 6);
  assert_eq!(word_backward(&mut editor, 10, false), 0);
}

#[test]
fn e_lands_on_the_start_of_the_last_character() {
  let mut editor = new_editor(&[LINE]);
  assert_eq!(word_end(&mut editor, 3, false), 10);
  assert_eq!(word_end(&mut editor, 12, false), 15);
  assert_eq!(word_end(&mut editor, 18, false), 21);
  assert_eq!(word_end(&mut editor, 12, true), 15);
  // Whitespace before the next word is skipped.
  assert_eq!(word_end(&mut editor, 11, false), 15);
}

#[test]
fn word_motions_cross_lines() {
  let mut editor = new_editor(&["añ", "está"]);
  // On the last character of the first line, `w` goes to the next line.
  assert_eq!(editor.lines[0].len(), 3);
  editor.cursor_x = CursorCol::lit(1);
  assert_eq!(editor.find_word_boundary(true, false).to_raw(), (1, 0));
  // `b` from the start of the second line goes to the first line's word.
  editor.cursor_y = 1;
  editor.cursor_x = CursorCol::lit(0);
  assert_eq!(editor.find_word_boundary(false, false).to_raw(), (0, 0));
}

#[test]
fn ascii_word_motions_are_unchanged() {
  let mut editor = new_editor(&["foo.bar baz"]);
  assert_eq!(word_forward(&mut editor, 0, false), 3);
  assert_eq!(word_forward(&mut editor, 3, false), 4);
  assert_eq!(word_forward(&mut editor, 0, true), 8);
  assert_eq!(word_backward(&mut editor, 8, false), 4);
  assert_eq!(word_end(&mut editor, 0, false), 2);
  assert_eq!(word_end(&mut editor, 0, true), 6);
}

#[test]
fn get_word_under_cursor_returns_whole_multibyte_words() {
  let mut editor = new_editor(&[LINE]);
  for (col, word) in
    [(3, "canción"), (8, "canción"), (9, "canción"), (15, "está")]
  {
    editor.cursor_x = CursorCol::lit(col);
    assert_eq!(editor.get_word_under_cursor().as_deref(), Some(word));
  }
  editor.cursor_x = CursorCol::lit(21);
  assert_eq!(editor.get_word_under_cursor().as_deref(), Some("aquí"));
  editor.cursor_x = CursorCol::lit(11);
  assert_eq!(editor.get_word_under_cursor(), None);

  let mut editor = new_editor(&["5€ 日本語"]);
  editor.cursor_x = CursorCol::lit(1);
  assert_eq!(editor.get_word_under_cursor(), None);
  editor.cursor_x = CursorCol::lit(8);
  assert_eq!(editor.get_word_under_cursor().as_deref(), Some("日本語"));
}

// "cafe\u{301}" ends in a combining accent that is not alphanumeric by
// itself, the Thai word is two clusters of three chars, the emoji are a ZWJ
// sequence and a flag, and 中文 is CJK.
const GRAPHEMES: &str = "cafe\u{301} ที่นี่ 👩\u{200d}💻🇪🇸 中文 x";

fn at(s: &str) -> usize {
  GRAPHEMES.find(s).unwrap()
}

#[test]
fn word_motions_treat_clusters_as_one_character() {
  let mut editor = new_editor(&[GRAPHEMES]);
  // w: the accent does not split "café"; the emoji run is one punctuation
  // word.
  let mut col = 0;
  for target in ["ที่", "👩", "中", "x"] {
    col = word_forward(&mut editor, col, false);
    assert_eq!(col, at(target), "w to {target:?}");
  }
  for target in ["中", "👩", "ที่", "c"] {
    col = word_backward(&mut editor, col, false);
    assert_eq!(col, at(target), "b to {target:?}");
  }
  // e lands on the start of the last cluster, never on a combining mark,
  // a tone mark or the tail of the ZWJ sequence.
  assert_eq!(word_end(&mut editor, 0, false), 3);
  assert_eq!(word_end(&mut editor, at("ที่"), false), at("นี่"));
  assert_eq!(word_end(&mut editor, at("👩"), false), at("🇪🇸"));
  assert_eq!(word_end(&mut editor, at("中"), false), at("文"));
  // From inside a cluster (on the accent, on the ZWJ), same as from its
  // start.
  assert_eq!(word_forward(&mut editor, 4, false), at("ที่"));
  assert_eq!(word_forward(&mut editor, at("👩") + 4, false), at("中"));
  assert_eq!(word_forward(&mut editor, at("👩"), true), at("中"));
}

#[test]
fn devanagari_words_are_whole() {
  let line = "नमस्ते दुनिया";
  let mut editor = new_editor(&[line]);
  assert_eq!(word_forward(&mut editor, 0, false), line.find("दु").unwrap());
  let end = word_end(&mut editor, 0, false);
  assert!(end > 0 && end < line.find(' ').unwrap());
  let end_col = crate::editor::char_boundary::bc(line, end);
  assert_eq!(end_col.floor_grapheme(line), end_col);
  editor.cursor_x = CursorCol::lit(3);
  assert_eq!(editor.get_word_under_cursor().as_deref(), Some("नमस्ते"));
}

#[test]
fn get_word_under_cursor_keeps_combining_marks() {
  let mut editor = new_editor(&[GRAPHEMES]);
  editor.cursor_x = CursorCol::lit(0);
  assert_eq!(editor.get_word_under_cursor().as_deref(), Some("cafe\u{301}"));
  editor.cursor_x = CursorCol::lit(at("นี่") + 3);
  assert_eq!(editor.get_word_under_cursor().as_deref(), Some("ที่นี่"));
  editor.cursor_x = CursorCol::lit(at("文"));
  assert_eq!(editor.get_word_under_cursor().as_deref(), Some("中文"));
  editor.cursor_x = CursorCol::lit(at("👩"));
  assert_eq!(editor.get_word_under_cursor(), None);
}
