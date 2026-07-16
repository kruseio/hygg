use super::super::core::Editor;
use cli_pdf_to_text::PdfLineKind;

fn rendered(buffer: Vec<u8>) -> String {
  String::from_utf8(buffer).expect("buffer should be UTF-8")
}

#[test]
fn buffered_content_writes_ansi_art_rows_without_highlighting() {
  let art = "\x1b[38;2;1;2;3m\x1b[48;2;4;5;6m▀\x1b[0m".to_string();
  let mut editor = Editor::new(vec!["text".to_string()], 80);
  // AnsiArt rows reach the reader through the streaming PDF installer
  // (rebuild_lines_from_pdf_stream), which writes self.lines directly — not
  // through Editor::new, whose sanitizer would (correctly) strip escapes from
  // untrusted plain text. Install the art the same way the real path does.
  editor.lines = vec!["text".to_string(), art.clone()];
  editor.height = 3;
  editor.line_kinds = vec![PdfLineKind::Text, PdfLineKind::AnsiArt];
  editor.show_highlighter = true;
  editor.cursor_y = 1;

  let mut buffer = Vec::new();
  editor
    .draw_content_buffered(&mut buffer, 80, "")
    .expect("draw should succeed");
  let output = rendered(buffer);

  assert!(output.contains(&art));
  assert!(output.contains("\x1b[0m"));
  assert!(!output.contains("\x1b[37m"));
}
