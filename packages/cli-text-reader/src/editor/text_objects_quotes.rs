use super::char_boundary::{Pos, char_starts};
use super::core::Editor;

impl Editor {
  // Select inner quotes (handles both " and ')
  pub fn select_inner_quotes(&mut self, quote_char: char) {
    let Pos { line: line_idx, col: col_idx } = self.get_cursor_position();

    if line_idx >= self.lines.len() {
      return;
    }

    let line = &self.lines[line_idx];

    // Find quote boundaries on current line
    let mut quotes = Vec::new();
    for (idx, ch) in char_starts(line) {
      if ch == quote_char {
        quotes.push(idx);
      }
    }

    // Find which pair we're in
    for i in (0..quotes.len()).step_by(2) {
      if i + 1 < quotes.len() {
        let start = quotes[i];
        let end = quotes[i + 1];

        if col_idx >= start && col_idx <= end {
          // We're inside this quote pair
          if end > start.next_char(line) {
            self.editor_state.selection_start =
              Some(Pos::new(line_idx, start.next_char(line)));
            self.editor_state.selection_end = Some(Pos::new(line_idx, end));

            // Update buffer selection
            if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
              buffer.selection_start = self.editor_state.selection_start;
              buffer.selection_end = self.editor_state.selection_end;
            }
          }
          return;
        }
      }
    }
  }

  // Select around quotes (includes the quotes)
  pub fn select_around_quotes(&mut self, quote_char: char) {
    let Pos { line: line_idx, col: col_idx } = self.get_cursor_position();

    if line_idx >= self.lines.len() {
      return;
    }

    let line = &self.lines[line_idx];

    // Find quote boundaries on current line
    let mut quotes = Vec::new();
    for (idx, ch) in char_starts(line) {
      if ch == quote_char {
        quotes.push(idx);
      }
    }

    // Find which pair we're in
    for i in (0..quotes.len()).step_by(2) {
      if i + 1 < quotes.len() {
        let start = quotes[i];
        let end = quotes[i + 1];

        if col_idx >= start && col_idx <= end {
          // We're inside this quote pair
          self.editor_state.selection_start = Some(Pos::new(line_idx, start));
          self.editor_state.selection_end =
            Some(Pos::new(line_idx, end.next_char(line)));

          // Update buffer selection
          if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
            buffer.selection_start = self.editor_state.selection_start;
            buffer.selection_end = self.editor_state.selection_end;
          }
          return;
        }
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::editor::char_boundary::CursorCol;

  #[test]
  fn quotes_around_accented_text_use_byte_offsets() {
    // " is byte 5, ó 11..13, closing " byte 14.
    let line = "dijo \"canción\" ya €";
    let mut editor = Editor::new(vec![line.to_string()], 80);
    editor.offset = 0;
    editor.cursor_y = 0;
    editor.cursor_x = CursorCol::lit(11);

    editor.select_inner_quotes('"');
    let (start, end) = (
      editor.editor_state.selection_start.unwrap().to_raw(),
      editor.editor_state.selection_end.unwrap().to_raw(),
    );
    assert_eq!((start, end), ((0, 6), (0, 14)));
    assert_eq!(&line[start.1..end.1], "canción");

    editor.select_around_quotes('"');
    let (start, end) = (
      editor.editor_state.selection_start.unwrap().to_raw(),
      editor.editor_state.selection_end.unwrap().to_raw(),
    );
    assert_eq!((start, end), ((0, 5), (0, 15)));
    assert_eq!(&line[start.1..end.1], "\"canción\"");
  }
}
