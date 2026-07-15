use super::core::Editor;

impl Editor {
  // Select inner paragraph
  pub fn select_inner_paragraph(&mut self) {
    let (line_idx, _) = self.get_cursor_position();

    // Find paragraph boundaries (empty lines)
    let mut start_line = line_idx;
    let mut end_line = line_idx;

    // Search backward for paragraph start
    while start_line > 0 {
      if self.lines[start_line - 1].trim().is_empty() {
        break;
      }
      start_line -= 1;
    }

    // Search forward for paragraph end
    while end_line < self.lines.len() - 1 {
      if self.lines[end_line + 1].trim().is_empty() {
        break;
      }
      end_line += 1;
    }

    // Set selection
    self.editor_state.selection_start = Some((start_line, 0));
    self.editor_state.selection_end =
      Some((end_line, self.lines[end_line].len()));

    // Update buffer selection
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.selection_start = self.editor_state.selection_start;
      buffer.selection_end = self.editor_state.selection_end;
    }
  }

  // Select around paragraph (includes surrounding empty lines)
  pub fn select_around_paragraph(&mut self) {
    let (line_idx, _) = self.get_cursor_position();

    // Find paragraph boundaries (empty lines)
    let mut start_line = line_idx;
    let mut end_line = line_idx;

    // Search backward for paragraph start
    while start_line > 0 {
      if self.lines[start_line - 1].trim().is_empty() {
        break;
      }
      start_line -= 1;
    }

    // Include empty lines before paragraph
    while start_line > 0 && self.lines[start_line - 1].trim().is_empty() {
      start_line -= 1;
    }

    // Search forward for paragraph end
    while end_line < self.lines.len() - 1 {
      if self.lines[end_line + 1].trim().is_empty() {
        break;
      }
      end_line += 1;
    }

    // Include empty lines after paragraph
    while end_line < self.lines.len() - 1
      && self.lines[end_line + 1].trim().is_empty()
    {
      end_line += 1;
    }

    // Set selection
    self.editor_state.selection_start = Some((start_line, 0));
    let end_col =
      if end_line < self.lines.len() { self.lines[end_line].len() } else { 0 };
    self.editor_state.selection_end = Some((end_line, end_col));

    // Update buffer selection
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.selection_start = self.editor_state.selection_start;
      buffer.selection_end = self.editor_state.selection_end;
    }
  }

  // Select inner sentence
  pub fn select_inner_sentence(&mut self) {
    let (line_idx, col_idx) = self.get_cursor_position();

    // Find sentence boundaries (. ! ?)
    let mut start_pos = (line_idx, col_idx);
    let mut end_pos = (line_idx, col_idx);

    // Search backward for sentence start
    let mut found_start = false;
    'outer: for search_line in (0..=line_idx).rev() {
      let line = &self.lines[search_line];
      let search_end =
        if search_line == line_idx { col_idx + 1 } else { line.len() };

      for search_col in (0..search_end).rev() {
        if let Some(ch) = line.chars().nth(search_col)
          && (ch == '.' || ch == '!' || ch == '?')
        {
          // Found sentence end, start is after this
          if search_col + 1 < line.len() {
            start_pos = (search_line, search_col + 1);
          } else if search_line + 1 < self.lines.len() {
            start_pos = (search_line + 1, 0);
          }
          // Skip whitespace after punctuation
          while start_pos.1 < self.lines[start_pos.0].len()
            && self.lines[start_pos.0]
              .chars()
              .nth(start_pos.1)
              .is_some_and(|c| c.is_whitespace())
          {
            start_pos.1 += 1;
          }
          found_start = true;
          break 'outer;
        }
      }
    }

    if !found_start {
      start_pos = (0, 0);
    }

    // Search forward for sentence end
    let mut found_end = false;
    'outer2: for search_line in line_idx..self.lines.len() {
      let line = &self.lines[search_line];
      let search_start = if search_line == line_idx { col_idx } else { 0 };

      for search_col in search_start..line.len() {
        if let Some(ch) = line.chars().nth(search_col)
          && (ch == '.' || ch == '!' || ch == '?')
        {
          end_pos = (search_line, search_col + 1);
          found_end = true;
          break 'outer2;
        }
      }
    }

    if !found_end && !self.lines.is_empty() {
      let last_line = self.lines.len() - 1;
      end_pos = (last_line, self.lines[last_line].len());
    }

    // Set selection
    self.editor_state.selection_start = Some(start_pos);
    self.editor_state.selection_end = Some(end_pos);

    // Update buffer selection
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.selection_start = self.editor_state.selection_start;
      buffer.selection_end = self.editor_state.selection_end;
    }
  }

  // Select around sentence (includes the punctuation and following space)
  pub fn select_around_sentence(&mut self) {
    // First select inner sentence
    self.select_inner_sentence();

    // Extend to include trailing whitespace
    if let Some((end_line, end_col)) = self.editor_state.selection_end {
      let mut new_end_col = end_col;

      if end_line < self.lines.len() {
        let line = &self.lines[end_line];
        while new_end_col < line.len()
          && line.chars().nth(new_end_col).is_some_and(|c| c.is_whitespace())
        {
          new_end_col += 1;
        }

        self.editor_state.selection_end = Some((end_line, new_end_col));

        // Update buffer selection
        if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
          buffer.selection_end = self.editor_state.selection_end;
        }
      }
    }
  }
}
