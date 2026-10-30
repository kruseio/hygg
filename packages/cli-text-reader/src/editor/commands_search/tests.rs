use super::*;
use crate::editor::char_boundary::CursorCol;
use cli_pdf_to_text::PdfLineKind;

#[test]
fn search_skips_ansi_art_rows() {
  let mut editor = Editor::new(
    vec![
      "alpha".to_string(),
      "\x1b[38;2;1;2;3m▀\x1b[0m".to_string(),
      "omega".to_string(),
    ],
    80,
  );
  editor.line_kinds =
    vec![PdfLineKind::Text, PdfLineKind::AnsiArt, PdfLineKind::Text];
  editor.cursor_y = 0;

  editor.editor_state.search_query = "38;2".to_string();
  editor.find_first_match(true);
  assert!(editor.editor_state.current_match.is_none());

  editor.editor_state.search_query = "omega".to_string();
  editor.find_first_match(true);
  assert_eq!(
    editor.editor_state.current_match.map(|m| (
      m.line,
      m.start.raw(),
      m.end.raw()
    )),
    Some((2, 0, 5))
  );
}

fn editor_at_top(lines: &[&str]) -> Editor {
  let mut editor =
    Editor::new(lines.iter().map(|l| l.to_string()).collect(), 80);
  editor.offset = 0;
  editor.cursor_y = 0;
  editor.cursor_x = CursorCol::lit(0);
  editor
}

#[test]
fn search_for_accented_text_lands_on_its_byte_offset() {
  let line = "La canción está aquí";
  let mut editor = editor_at_top(&[line]);
  editor.editor_state.search_query = "ción".to_string();
  editor.find_first_match(true);
  assert_eq!(
    editor.editor_state.current_match.map(|m| (
      m.line,
      m.start.raw(),
      m.end.raw()
    )),
    Some((0, 6, 11))
  );
  editor.center_on_match();
  assert_eq!(editor.cursor_x.raw(), 6);
  assert_eq!(&line[6..11], "ción");
}

#[test]
fn search_preview_for_accented_text_lands_on_its_byte_offset() {
  let mut editor = editor_at_top(&["La canción está aquí"]);
  editor.find_preview_match("CIÓN", true);
  assert_eq!(
    editor.editor_state.search_preview_match.map(|m| (
      m.line,
      m.start.raw(),
      m.end.raw()
    )),
    Some((0, 6, 11))
  );
  assert_eq!(editor.cursor_x.raw(), 6);
}

#[test]
fn length_changing_lowercase_before_the_match_keeps_offsets_exact() {
  // 'İ' is 2 bytes but lowercases to 3, which used to shift every match
  // after it by one byte (and, here, into the middle of 'ó').
  let line = "İstanbul: la canción";
  let mut editor = editor_at_top(&["first", line]);
  editor.editor_state.search_query = "canción".to_string();
  editor.find_first_match(true);
  let (row, start, end) = editor
    .editor_state
    .current_match
    .map(|m| (m.line, m.start.raw(), m.end.raw()))
    .unwrap();
  assert_eq!(row, 1);
  assert_eq!(&line[start..end], "canción");
  editor.center_on_match();
  assert_eq!(editor.offset + editor.cursor_y, 1);
  assert_eq!(editor.cursor_x.raw(), start);

  // Backward search and preview go through the same mapping.
  editor.offset = 0;
  editor.cursor_y = 0;
  editor.editor_state.current_match = None;
  editor.find_next_match(false);
  let (_, s, e) = editor
    .editor_state
    .current_match
    .map(|m| (m.line, m.start.raw(), m.end.raw()))
    .unwrap();
  assert_eq!(&line[s..e], "canción");

  let mut editor = editor_at_top(&["first", line]);
  editor.find_preview_match("ción", true);
  let (_, s, e) = editor
    .editor_state
    .search_preview_match
    .map(|m| (m.line, m.start.raw(), m.end.raw()))
    .unwrap();
  assert_eq!(&line[s..e], "ción");
}

#[test]
fn search_from_a_mid_char_cursor_does_not_panic() {
  let line = "La canción está aquí canción";
  let mut editor = editor_at_top(&[line]);
  // Byte 9 is inside the first 'ó' (8..10).
  editor.cursor_x = CursorCol::lit(9);
  editor.editor_state.search_query = "ción".to_string();
  editor.find_first_match(true);
  assert_eq!(
    editor.editor_state.current_match.map(|m| (
      m.line,
      m.start.raw(),
      m.end.raw()
    )),
    Some((0, 27, 32))
  );
  // Byte 30 is inside the second 'ó' (29..31); the prefix before it ends in
  // "canci".
  editor.cursor_x = CursorCol::lit(30);
  editor.editor_state.search_query = "canci".to_string();
  editor.find_first_match(false);
  assert_eq!(
    editor.editor_state.current_match.map(|m| (
      m.line,
      m.start.raw(),
      m.end.raw()
    )),
    Some((0, 24, 29))
  );
}
