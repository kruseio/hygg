use super::char_boundary::{ByteCol, Pos, char_starts};
use super::core::Editor;

impl Editor {
  // Select inner content between matching delimiters
  pub(super) fn select_between_delimiters(
    &mut self,
    open_delim: char,
    close_delim: char,
    include_delimiters: bool,
  ) {
    let cursor = self.get_cursor_position();

    // Find the opening delimiter
    let Some(open) =
      self.find_delimiter_backward(open_delim, close_delim, cursor)
    else {
      return;
    };

    // Find the closing delimiter
    let Some(close) =
      self.find_delimiter_forward(close_delim, open_delim, cursor)
    else {
      return;
    };

    // Just past each delimiter, a whole character on its own line.
    let after = |pos: Pos| {
      let line = self.lines.get(pos.line).map_or("", String::as_str);
      Pos::new(pos.line, pos.col.next_char(line))
    };
    let (after_open, after_close) = (after(open), after(close));

    // Set selection based on whether we include delimiters
    if include_delimiters {
      // Around - include the delimiters
      self.editor_state.selection_start = Some(open);
      self.editor_state.selection_end = Some(after_close);
    } else {
      // Inner - exclude the delimiters
      if open.line == close.line {
        // Same line
        if close.col > after_open.col {
          self.editor_state.selection_start = Some(after_open);
          self.editor_state.selection_end = Some(close);
        }
      } else {
        // Multi-line
        self.editor_state.selection_start = Some(after_open);
        self.editor_state.selection_end = Some(close);
      }
    }

    // Update buffer selection
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.selection_start = self.editor_state.selection_start;
      buffer.selection_end = self.editor_state.selection_end;
    }
  }

  // Find delimiter backward from `from`, the cursor's own character included
  fn find_delimiter_backward(
    &self,
    target: char,
    opposite: char,
    from: Pos,
  ) -> Option<Pos> {
    let mut nesting = 0;

    // Search backward from current position
    for line_idx in (0..=from.line).rev() {
      if line_idx >= self.lines.len() {
        continue;
      }

      let line = &self.lines[line_idx];
      // Include the cursor's whole character (issue #99).
      let end_col = if line_idx == from.line {
        from.col.next_char(line)
      } else {
        ByteCol::end(line)
      };

      for (col_idx, ch) in char_starts(line).rev().filter(|&(i, _)| i < end_col)
      {
        if ch == opposite {
          nesting += 1;
        } else if ch == target {
          if nesting == 0 {
            return Some(Pos::new(line_idx, col_idx));
          }
          nesting -= 1;
        }
      }
    }

    None
  }

  // Find delimiter forward from `from`, the cursor's own character included
  fn find_delimiter_forward(
    &self,
    target: char,
    opposite: char,
    from: Pos,
  ) -> Option<Pos> {
    let mut nesting = 0;

    // Search forward from current position
    for line_idx in from.line..self.lines.len() {
      let line = &self.lines[line_idx];
      let start_col = if line_idx == from.line {
        from.col.settle(line)
      } else {
        ByteCol::ZERO
      };

      for (col_idx, ch) in char_starts(line).filter(|&(i, _)| i >= start_col) {
        if ch == opposite {
          nesting += 1;
        } else if ch == target {
          if nesting == 0 {
            return Some(Pos::new(line_idx, col_idx));
          }
          nesting -= 1;
        }
      }
    }

    None
  }

  // Select inner braces {}
  pub fn select_inner_braces(&mut self) {
    self.select_between_delimiters('{', '}', false);
  }

  // Select around braces {}
  pub fn select_around_braces(&mut self) {
    self.select_between_delimiters('{', '}', true);
  }

  // Select inner parentheses ()
  pub fn select_inner_parentheses(&mut self) {
    self.select_between_delimiters('(', ')', false);
  }

  // Select around parentheses ()
  pub fn select_around_parentheses(&mut self) {
    self.select_between_delimiters('(', ')', true);
  }

  // Select inner brackets []
  pub fn select_inner_brackets(&mut self) {
    self.select_between_delimiters('[', ']', false);
  }

  // Select around brackets []
  pub fn select_around_brackets(&mut self) {
    self.select_between_delimiters('[', ']', true);
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::editor::char_boundary::{CursorCol, Pos};

  type Selection = Option<(usize, usize)>;

  // d0 i1 j2 o3 ' '4 (5 c6 a7 n8 c9 i10 ó11..13 n13 ' '14 €15..18 )18 ' '19
  const LINE: &str = "dijo (canción €) ya";

  fn new_editor(lines: &[&str]) -> Editor {
    let mut editor =
      Editor::new(lines.iter().map(|l| l.to_string()).collect(), 80);
    editor.height = 10;
    editor.offset = 0;
    editor.cursor_y = 0;
    editor
  }

  fn selection(editor: &Editor) -> (Selection, Selection) {
    (
      editor.editor_state.selection_start.map(Pos::to_raw),
      editor.editor_state.selection_end.map(Pos::to_raw),
    )
  }

  #[test]
  fn inner_and_around_parens_with_cursor_on_multibyte_chars() {
    for col in [11, 15, 6] {
      let mut editor = new_editor(&[LINE]);
      editor.cursor_x = CursorCol::lit(col);
      editor.select_inner_parentheses();
      assert_eq!(selection(&editor), (Some((0, 6)), Some((0, 18))), "{col}");
      assert_eq!(&LINE[6..18], "canción €");

      let mut editor = new_editor(&[LINE]);
      editor.cursor_x = CursorCol::lit(col);
      editor.select_around_parentheses();
      assert_eq!(selection(&editor), (Some((0, 5)), Some((0, 19))), "{col}");
    }
  }

  #[test]
  fn delimiters_across_lines_with_multibyte_chars() {
    // Line 1: € 0..3, ' ' 3, } 4.
    let mut editor = new_editor(&["{ é", "€ }"]);
    editor.cursor_x = CursorCol::lit(2);
    editor.select_inner_braces();
    assert_eq!(selection(&editor), (Some((0, 1)), Some((1, 4))));
  }

  #[test]
  fn cursor_past_the_end_does_not_panic() {
    let mut editor = new_editor(&["(añ€"]);
    editor.cursor_x = CursorCol::lit(99);
    editor.select_inner_parentheses();
    assert_eq!(selection(&editor), (None, None));

    let mut editor = new_editor(&["[añ€]"]);
    editor.cursor_x = CursorCol::lit(4);
    editor.select_inner_brackets();
    assert_eq!(selection(&editor), (Some((0, 1)), Some((0, 7))));
  }
}
