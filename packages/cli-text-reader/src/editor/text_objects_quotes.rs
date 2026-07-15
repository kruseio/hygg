use super::core::Editor;

impl Editor {
  // Select inner quotes (handles both " and ')
  pub fn select_inner_quotes(&mut self, quote_char: char) {
    let (line_idx, col_idx) = self.get_cursor_position();

    if line_idx >= self.lines.len() {
      return;
    }

    let line = &self.lines[line_idx];

    // Find quote boundaries on current line
    let mut quotes = Vec::new();
    for (idx, ch) in line.char_indices() {
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
          if end > start + 1 {
            self.editor_state.selection_start = Some((line_idx, start + 1));
            self.editor_state.selection_end = Some((line_idx, end));

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
    let (line_idx, col_idx) = self.get_cursor_position();

    if line_idx >= self.lines.len() {
      return;
    }

    let line = &self.lines[line_idx];

    // Find quote boundaries on current line
    let mut quotes = Vec::new();
    for (idx, ch) in line.char_indices() {
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
          self.editor_state.selection_start = Some((line_idx, start));
          self.editor_state.selection_end = Some((line_idx, end + 1));

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
