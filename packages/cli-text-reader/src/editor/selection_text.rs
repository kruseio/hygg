use super::char_boundary::{Pos, slice, slice_from, slice_to};
use super::core::Editor;

impl Editor {
  // Extract selected text based on start and end positions
  pub fn get_selected_text(&self) -> String {
    if let (Some(start), Some(end)) =
      (self.editor_state.selection_start, self.editor_state.selection_end)
    {
      let Pos { line: start_line, col: start_col } =
        if start.line <= end.line { start } else { end };

      let Pos { line: end_line, col: end_col } =
        if start.line <= end.line { end } else { start };

      // Handle visual line mode vs character mode differently
      if self.editor_state.mode == super::core::EditorMode::VisualLine {
        // In line mode, select entire lines
        if start_line < self.lines.len() && end_line < self.lines.len() {
          return (start_line..=end_line)
            .filter(|line| !self.is_ansi_art_line(*line))
            .map(|line| self.lines[line].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        }
      } else {
        // In character mode, select parts of lines
        if start_line == end_line {
          // Single line selection
          if start_line < self.lines.len() {
            if self.is_ansi_art_line(start_line) {
              return String::new();
            }
            // Columns are byte offsets; clamp them into the line and onto
            // character boundaries so a stale or mid-char column never
            // panics.
            let line = &self.lines[start_line];
            let (lo, hi) = (start_col.min(end_col), start_col.max(end_col));
            return slice(line, lo, hi).to_string();
          }
        } else {
          // Multi-line selection
          let mut result = String::new();

          // First line (partial)
          if start_line < self.lines.len() && !self.is_ansi_art_line(start_line)
          {
            let line = &self.lines[start_line];
            result.push_str(slice_from(line, start_col));
            result.push('\n');
          }

          // Middle lines (full)
          for i in start_line + 1..end_line {
            if i < self.lines.len() && !self.is_ansi_art_line(i) {
              result.push_str(&self.lines[i]);
              result.push('\n');
            }
          }

          // Last line (partial)
          if end_line < self.lines.len() && !self.is_ansi_art_line(end_line) {
            let line = &self.lines[end_line];
            result.push_str(slice_to(line, end_col));
          }

          return result;
        }
      }
    }

    String::new()
  }
}

#[cfg(test)]
mod tests {
  use crate::editor::char_boundary::{ByteCol, Pos};
  use crate::editor::core::{Editor, EditorMode};

  // "La canción está aquí": 'ó' is bytes 8..10, 'á' 15..17, 'í' 21..23.
  fn editor_with(lines: &[&str]) -> Editor {
    let mut editor =
      Editor::new(lines.iter().map(|l| l.to_string()).collect(), 80);
    editor.offset = 0;
    editor.cursor_y = 0;
    editor.set_active_mode(EditorMode::VisualChar);
    editor
  }

  // Literal, possibly stale (mid-character or past the end) selection ends.
  fn select(editor: &mut Editor, start: (usize, usize), end: (usize, usize)) {
    let at = |(line, col): (usize, usize)| Pos::new(line, ByteCol::lit(col));
    editor.editor_state.selection_start = Some(at(start));
    editor.editor_state.selection_end = Some(at(end));
  }

  #[test]
  fn single_line_selection_with_mid_char_columns_does_not_panic() {
    let mut editor = editor_with(&["La canción está aquí"]);
    // End inside 'ó' floors to its start.
    select(&mut editor, (0, 3), (0, 9));
    assert_eq!(editor.get_selected_text(), "canci");
    // Start inside 'ó' floors to its start; reversed columns still work.
    select(&mut editor, (0, 11), (0, 9));
    assert_eq!(editor.get_selected_text(), "ón");
    // Past-the-end columns clamp to the line.
    select(&mut editor, (0, 18), (0, 99));
    assert_eq!(editor.get_selected_text(), "aquí");
  }

  #[test]
  fn multi_line_selection_with_mid_char_columns_does_not_panic() {
    let mut editor = editor_with(&["La canción está aquí", "precio 5€ más"]);
    // '€' is bytes 8..11 of the second line; 'á' is 13..15.
    select(&mut editor, (0, 16), (1, 10));
    assert_eq!(editor.get_selected_text(), "á aquí\nprecio 5");
    select(&mut editor, (0, 99), (1, 14));
    assert_eq!(editor.get_selected_text(), "\nprecio 5€ m");
    select(&mut editor, (0, 0), (1, 99));
    assert_eq!(
      editor.get_selected_text(),
      "La canción está aquí\nprecio 5€ más"
    );
  }
}
