use super::*;
use crate::editor::char_boundary::{ByteCol, CursorCol, Pos};

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
fn right_and_left_step_whole_characters() {
  let mut editor = new_editor(&[LINE]);
  let starts: Vec<usize> = LINE.char_indices().map(|(i, _)| i).collect();

  let mut seen = vec![editor.cursor_x.raw()];
  for _ in 0..LINE.len() {
    editor.move_cursor_right();
    if seen.last() != Some(&editor.cursor_x.raw()) {
      seen.push(editor.cursor_x.raw());
    }
  }
  assert_eq!(seen, starts);
  assert_eq!(editor.cursor_x.raw(), 21);

  let mut seen = vec![editor.cursor_x.raw()];
  for _ in 0..LINE.len() {
    editor.move_cursor_left();
    if seen.last() != Some(&editor.cursor_x.raw()) {
      seen.push(editor.cursor_x.raw());
    }
  }
  seen.reverse();
  assert_eq!(seen, starts);
  assert_eq!(editor.cursor_x.raw(), 0);
}

#[test]
fn right_and_left_over_three_byte_and_cjk_chars() {
  let mut editor = new_editor(&["€日x"]);
  editor.move_cursor_right();
  assert_eq!(editor.cursor_x.raw(), 3);
  editor.move_cursor_right();
  assert_eq!(editor.cursor_x.raw(), 6);
  editor.move_cursor_right();
  assert_eq!(editor.cursor_x.raw(), 6);
  editor.move_cursor_left();
  assert_eq!(editor.cursor_x.raw(), 3);
  editor.move_cursor_left();
  assert_eq!(editor.cursor_x.raw(), 0);
}

#[test]
fn up_and_down_keep_the_byte_line_mapping_sane_on_accents() {
  // "añb": a 0, ñ 1..3, b 3. "€€€€": € at 0, 3, 6, 9.
  let mut editor = new_editor(&[LINE, "añb", "€€€€", ""]);
  // The 'a' of "canción" is display column 4: clamps to 'b' (column 2).
  editor.cursor_x = CursorCol::lit(4);
  editor.move_cursor_down();
  assert_eq!((editor.cursor_y, editor.cursor_x.raw()), (1, 3));
  // The remembered column 4 is restored on the next line: the fifth '€'
  // does not exist, so the last one; and on an empty line, 0.
  editor.move_cursor_down();
  assert_eq!((editor.cursor_y, editor.cursor_x.raw()), (2, 9));
  editor.move_cursor_down();
  assert_eq!((editor.cursor_y, editor.cursor_x.raw()), (3, 0));
  // Back up, all the way to column 4 on the first line.
  editor.move_cursor_up();
  editor.move_cursor_up();
  editor.move_cursor_up();
  assert_eq!((editor.cursor_y, editor.cursor_x.raw()), (0, 4));
}

#[test]
fn move_to_position_floors_onto_a_boundary() {
  let mut editor = new_editor(&[LINE, "añb"]);
  editor.move_to_position(Pos::new(0, ByteCol::lit(9)));
  assert_eq!(editor.cursor_x.raw(), 8);
  editor.move_to_position(Pos::new(1, ByteCol::lit(2)));
  assert_eq!(editor.offset + editor.cursor_y, 1);
  assert_eq!(editor.cursor_x.raw(), 1);
  editor.move_to_position(Pos::new(1, ByteCol::lit(3)));
  assert_eq!(editor.cursor_x.raw(), 3);
}

#[test]
fn move_to_word_boundary_walks_accented_words() {
  let mut editor = new_editor(&[LINE, "日本 x"]);
  let mut stops = vec![];
  for _ in 0..5 {
    editor.move_to_word_boundary(true, false);
    stops.push((editor.offset + editor.cursor_y, editor.cursor_x.raw()));
  }
  assert_eq!(stops, vec![(0, 3), (0, 12), (0, 18), (1, 0), (1, 7)]);

  let mut stops = vec![];
  for _ in 0..4 {
    editor.move_to_word_boundary(false, false);
    stops.push((editor.offset + editor.cursor_y, editor.cursor_x.raw()));
  }
  assert_eq!(stops, vec![(1, 0), (0, 18), (0, 12), (0, 3)]);
}

// One line holding every kind of multi-char or wide grapheme.
fn mixed_line() -> String {
  ["a", "e\u{301}", "ที่", "कि", "👩\u{200d}💻", "🇪🇸", "中", "Ａ", "z"].concat()
}

fn grapheme_starts(line: &str) -> Vec<usize> {
  use unicode_segmentation::UnicodeSegmentation;
  line.grapheme_indices(true).map(|(i, _)| i).collect()
}

#[test]
fn h_and_l_step_whole_grapheme_clusters() {
  let line = mixed_line();
  let starts = grapheme_starts(&line);
  assert_eq!(starts.len(), 9);
  let mut editor = new_editor(&[&line]);

  let mut seen = vec![editor.cursor_x.raw()];
  for _ in 0..line.len() {
    editor.move_cursor_right();
    if seen.last() != Some(&editor.cursor_x.raw()) {
      seen.push(editor.cursor_x.raw());
    }
  }
  assert_eq!(seen, starts);

  let mut seen = vec![editor.cursor_x.raw()];
  for _ in 0..line.len() {
    editor.move_cursor_left();
    if seen.last() != Some(&editor.cursor_x.raw()) {
      seen.push(editor.cursor_x.raw());
    }
  }
  seen.reverse();
  assert_eq!(seen, starts);
}

#[test]
fn arrow_keys_and_l_step_graphemes_through_the_key_handler() {
  use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
  let line = mixed_line();
  let starts = grapheme_starts(&line);
  let mut editor = new_editor(&[&line]);
  let press = |editor: &mut Editor, code| {
    editor
      .handle_normal_mode_event(KeyEvent::new(code, KeyModifiers::NONE))
      .unwrap();
  };
  for &expected in &starts[1..] {
    press(&mut editor, KeyCode::Right);
    assert_eq!(editor.cursor_x.raw(), expected);
  }
  press(&mut editor, KeyCode::Left);
  assert_eq!(editor.cursor_x.raw(), starts[7]);
  press(&mut editor, KeyCode::Char('h'));
  assert_eq!(editor.cursor_x.raw(), starts[6]);
  press(&mut editor, KeyCode::Char('l'));
  assert_eq!(editor.cursor_x.raw(), starts[7]);
}

#[test]
fn j_and_k_keep_the_display_column_across_wide_and_combining_text() {
  use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
  // Display columns of each line:
  // - "abcdefgh": one cell each.
  // - "中文字漢": two cells each, at 0, 2, 4 and 6.
  // - "ab": short.
  // - "ae\u{301}ที่xyz": a 0, é 1, ที่ 2, x 3, y 4, z 5, at bytes 0, 1, 4, 13, 14
  //   and 15.
  // - "ＡＢＣ👩\u{200d}💻🇪🇸": fullwidth and emoji, two cells each.
  let lines =
    ["abcdefgh", "中文字漢", "ab", "ae\u{301}ที่xyz", "ＡＢＣ👩\u{200d}💻🇪🇸"];
  let mut editor = new_editor(&lines);
  let press = |editor: &mut Editor, c| {
    editor
      .handle_normal_mode_event(KeyEvent::new(
        KeyCode::Char(c),
        KeyModifiers::NONE,
      ))
      .unwrap();
    (editor.offset + editor.cursor_y, editor.cursor_x.raw())
  };
  // To 'f', display column 5.
  for _ in 0..5 {
    press(&mut editor, 'l');
  }
  assert_eq!(editor.cursor_x.raw(), 5);
  // Column 5 is the right half of 字 (columns 4-5): land on 字.
  assert_eq!(press(&mut editor, 'j'), (1, 6));
  assert_eq!(editor.cursor_screen_column(), 4);
  // Short line: clamp to its last character.
  assert_eq!(press(&mut editor, 'j'), (2, 1));
  // Column 5 again, past a combining accent and a Thai cluster.
  assert_eq!(press(&mut editor, 'j'), (3, 15));
  // Column 5 is the right half of 'Ｃ' (columns 4-5).
  assert_eq!(press(&mut editor, 'j'), (4, 6));
  // And all the way back up to 'f'.
  assert_eq!(press(&mut editor, 'k'), (3, 15));
  assert_eq!(press(&mut editor, 'k'), (2, 1));
  assert_eq!(press(&mut editor, 'k'), (1, 6));
  assert_eq!(press(&mut editor, 'k'), (0, 5));

  // A horizontal motion sets a new column: 'h' to 'e' (column 4), then
  // down lands on 字 (columns 4-5) and on the 'y' at column 4 further on.
  press(&mut editor, 'h');
  assert_eq!(press(&mut editor, 'j'), (1, 6));
  assert_eq!(press(&mut editor, 'j'), (2, 1));
  assert_eq!(press(&mut editor, 'j'), (3, 14));
  // The emoji line: column 4 is 'Ｃ'.
  assert_eq!(press(&mut editor, 'j'), (4, 6));
  // Column 6 on the emoji line is the ZWJ sequence; up from there goes to
  // the 'z' at column 5, the line's last grapheme.
  press(&mut editor, 'l');
  assert_eq!(editor.cursor_x.raw(), 9);
  assert_eq!(editor.cursor_screen_column(), 6);
  assert_eq!(press(&mut editor, 'k'), (3, 15));
  assert_eq!(press(&mut editor, 'k'), (2, 1));
  assert_eq!(press(&mut editor, 'k'), (1, 9));
  assert_eq!(press(&mut editor, 'k'), (0, 6));
}

#[test]
fn dollar_makes_j_and_k_stay_at_the_end_of_each_line() {
  use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
  let lines = ["ab👩\u{200d}💻", "x🇪🇸🇪🇸", "e\u{301}", "ที่นี่ 中"];
  let mut editor = new_editor(&lines);
  let press = |editor: &mut Editor, c| {
    editor
      .handle_normal_mode_event(KeyEvent::new(
        KeyCode::Char(c),
        KeyModifiers::NONE,
      ))
      .unwrap();
    editor.cursor_x.raw()
  };
  assert_eq!(press(&mut editor, '$'), 2);
  assert_eq!(press(&mut editor, 'j'), 9);
  assert_eq!(press(&mut editor, 'j'), 0);
  assert_eq!(press(&mut editor, 'j'), lines[3].find('中').unwrap());
  assert_eq!(press(&mut editor, 'k'), 0);
  assert_eq!(press(&mut editor, 'k'), 9);
  assert_eq!(press(&mut editor, 'k'), 2);
}

#[test]
fn move_to_position_floors_onto_a_grapheme_start() {
  // x 0, 👩 1..5, ZWJ 5..8, 💻 8..12, e 12, U+0301 13..15
  let mut editor = new_editor(&["x👩\u{200d}💻e\u{301}"]);
  for (target, expected) in [(5, 1), (8, 1), (11, 1), (13, 12), (12, 12)] {
    editor.move_to_position(Pos::new(0, ByteCol::lit(target)));
    assert_eq!(editor.cursor_x.raw(), expected, "target {target}");
  }
}
