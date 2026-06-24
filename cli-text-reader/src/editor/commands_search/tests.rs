use super::*;
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
  assert_eq!(editor.editor_state.current_match, Some((2, 0, 5)));
}
