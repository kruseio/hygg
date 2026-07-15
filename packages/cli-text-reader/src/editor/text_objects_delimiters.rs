use super::core::Editor;

impl Editor {
  // Select inner content between matching delimiters
  pub(super) fn select_between_delimiters(
    &mut self,
    open_delim: char,
    close_delim: char,
    include_delimiters: bool,
  ) {
    let (line_idx, col_idx) = self.get_cursor_position();

    // Find the opening delimiter
    let (open_line, open_col) = if let Some(pos) =
      self.find_delimiter_backward(open_delim, close_delim, line_idx, col_idx)
    {
      pos
    } else {
      return;
    };

    // Find the closing delimiter
    let (close_line, close_col) = if let Some(pos) =
      self.find_delimiter_forward(close_delim, open_delim, line_idx, col_idx)
    {
      pos
    } else {
      return;
    };

    // Set selection based on whether we include delimiters
    if include_delimiters {
      // Around - include the delimiters
      self.editor_state.selection_start = Some((open_line, open_col));
      self.editor_state.selection_end = Some((close_line, close_col + 1));
    } else {
      // Inner - exclude the delimiters
      if open_line == close_line {
        // Same line
        if close_col > open_col + 1 {
          self.editor_state.selection_start = Some((open_line, open_col + 1));
          self.editor_state.selection_end = Some((close_line, close_col));
        }
      } else {
        // Multi-line
        self.editor_state.selection_start = Some((open_line, open_col + 1));
        self.editor_state.selection_end = Some((close_line, close_col));
      }
    }

    // Update buffer selection
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.selection_start = self.editor_state.selection_start;
      buffer.selection_end = self.editor_state.selection_end;
    }
  }

  // Find delimiter backward from current position
  fn find_delimiter_backward(
    &self,
    target: char,
    opposite: char,
    start_line: usize,
    start_col: usize,
  ) -> Option<(usize, usize)> {
    let mut nesting = 0;

    // Search backward from current position
    for line_idx in (0..=start_line).rev() {
      if line_idx >= self.lines.len() {
        continue;
      }

      let line = &self.lines[line_idx];
      let end_col =
        if line_idx == start_line { start_col + 1 } else { line.len() };

      for (col_idx, ch) in line[..end_col].char_indices().rev() {
        if ch == opposite {
          nesting += 1;
        } else if ch == target {
          if nesting == 0 {
            return Some((line_idx, col_idx));
          }
          nesting -= 1;
        }
      }
    }

    None
  }

  // Find delimiter forward from current position
  fn find_delimiter_forward(
    &self,
    target: char,
    opposite: char,
    start_line: usize,
    start_col: usize,
  ) -> Option<(usize, usize)> {
    let mut nesting = 0;

    // Search forward from current position
    for line_idx in start_line..self.lines.len() {
      let line = &self.lines[line_idx];
      let start_col_in_line =
        if line_idx == start_line { start_col } else { 0 };

      for (col_idx, ch) in line[start_col_in_line..].char_indices() {
        let actual_col = start_col_in_line + col_idx;
        if ch == opposite {
          nesting += 1;
        } else if ch == target {
          if nesting == 0 {
            return Some((line_idx, actual_col));
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
