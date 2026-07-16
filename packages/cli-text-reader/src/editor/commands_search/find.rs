use super::super::core::Editor;

impl Editor {
  // Find first match from current cursor position (for new searches)
  #[allow(dead_code)]
  pub fn find_first_match(&mut self, forward: bool) {
    if self.editor_state.search_query.is_empty() {
      return;
    }

    let query = self.editor_state.search_query.to_lowercase();
    let current_line = self.offset + self.cursor_y;

    // Clear any existing match to ensure we search from cursor position
    self.editor_state.current_match = None;

    let find_in_line = |line: &str, query: &str| -> Option<(usize, usize)> {
      line.to_lowercase().find(query).map(|start| (start, start + query.len()))
    };

    if forward {
      // First check current line from cursor position onward
      if current_line < self.lines.len() && !self.is_ansi_art_line(current_line)
      {
        let line = &self.lines[current_line];
        if self.cursor_x < line.len() {
          let remaining = &line[self.cursor_x..];
          if let Some(pos) = remaining.to_lowercase().find(&query) {
            let start = self.cursor_x + pos;
            let end = start + query.len();
            self.editor_state.current_match = Some((current_line, start, end));
            return;
          }
        }
      }

      // Then search forward from next line
      for i in current_line + 1..self.lines.len() {
        if self.is_ansi_art_line(i) {
          continue;
        }
        if let Some((start, end)) = find_in_line(&self.lines[i], &query) {
          self.editor_state.current_match = Some((i, start, end));
          // Also update active buffer's current_match
          if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
            buffer.current_match = Some((i, start, end));
          }
          return;
        }
      }
      // Wrap around to beginning
      for i in 0..=current_line {
        if self.is_ansi_art_line(i) {
          continue;
        }
        if let Some((start, end)) = find_in_line(&self.lines[i], &query) {
          self.editor_state.current_match = Some((i, start, end));
          // Also update active buffer's current_match
          if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
            buffer.current_match = Some((i, start, end));
          }
          return;
        }
      }
    } else {
      // First check current line from cursor position backward
      if current_line < self.lines.len()
        && self.cursor_x > 0
        && !self.is_ansi_art_line(current_line)
      {
        let line = &self.lines[current_line];
        let before_cursor = &line[..self.cursor_x];
        if let Some(pos) = before_cursor.to_lowercase().rfind(&query) {
          let end = pos + query.len();
          self.editor_state.current_match = Some((current_line, pos, end));
          return;
        }
      }

      // Then search backward from previous line
      for i in (0..current_line).rev() {
        if self.is_ansi_art_line(i) {
          continue;
        }
        if let Some((start, end)) = find_in_line(&self.lines[i], &query) {
          self.editor_state.current_match = Some((i, start, end));
          // Also update active buffer's current_match
          if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
            buffer.current_match = Some((i, start, end));
          }
          return;
        }
      }
      // Wrap around to end
      for i in (current_line..self.lines.len()).rev() {
        if self.is_ansi_art_line(i) {
          continue;
        }
        if let Some((start, end)) = find_in_line(&self.lines[i], &query) {
          self.editor_state.current_match = Some((i, start, end));
          // Also update active buffer's current_match
          if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
            buffer.current_match = Some((i, start, end));
          }
          return;
        }
      }
    }
  }

  pub fn find_next_match(&mut self, forward: bool) {
    if self.editor_state.search_query.is_empty() {
      return;
    }

    let query = self.editor_state.search_query.to_lowercase();
    let start_idx = if let Some((idx, _, _)) = self.editor_state.current_match {
      idx
    } else {
      self.offset + self.cursor_y
    };

    let find_in_line = |line: &str, query: &str| -> Option<(usize, usize)> {
      line.to_lowercase().find(query).map(|start| (start, start + query.len()))
    };

    let find_in_line_backward = |line: &str,
                                 query: &str|
     -> Option<(usize, usize)> {
      line.to_lowercase().rfind(query).map(|start| (start, start + query.len()))
    };

    if forward {
      // Forward search
      for i in start_idx + 1..self.lines.len() {
        if self.is_ansi_art_line(i) {
          continue;
        }
        if let Some((start, end)) = find_in_line(&self.lines[i], &query) {
          self.editor_state.current_match = Some((i, start, end));
          // Also update active buffer's current_match
          if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
            buffer.current_match = Some((i, start, end));
          }
          return;
        }
      }
      // Wrap around to beginning
      for i in 0..=start_idx {
        if self.is_ansi_art_line(i) {
          continue;
        }
        if let Some((start, end)) = find_in_line(&self.lines[i], &query) {
          self.editor_state.current_match = Some((i, start, end));
          // Also update active buffer's current_match
          if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
            buffer.current_match = Some((i, start, end));
          }
          return;
        }
      }
    } else {
      // Backward search - use rfind to get last occurrence in each line
      for i in (0..start_idx).rev() {
        if self.is_ansi_art_line(i) {
          continue;
        }
        if let Some((start, end)) =
          find_in_line_backward(&self.lines[i], &query)
        {
          self.editor_state.current_match = Some((i, start, end));
          // Also update active buffer's current_match
          if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
            buffer.current_match = Some((i, start, end));
          }
          return;
        }
      }
      // Wrap around to end
      for i in (start_idx..self.lines.len()).rev() {
        if self.is_ansi_art_line(i) {
          continue;
        }
        if let Some((start, end)) =
          find_in_line_backward(&self.lines[i], &query)
        {
          self.editor_state.current_match = Some((i, start, end));
          // Also update active buffer's current_match
          if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
            buffer.current_match = Some((i, start, end));
          }
          return;
        }
      }
    }
  }

  // Center the view on the current search match and move cursor to it
  pub fn center_on_match(&mut self) {
    if let Some((line_idx, col_idx, _)) = self.editor_state.current_match {
      // Center the view
      let content_height = self.height.saturating_sub(1);
      let half_height = (content_height / 2) as i32;
      let new_offset = line_idx as i32 - half_height;
      self.offset = if new_offset < 0 {
        0
      } else if new_offset + content_height as i32 > self.total_lines as i32 {
        // A document shorter than the viewport (total_lines < content_height)
        // with a match near its end reaches here and underflowed: panic in
        // debug, a wildly wrong offset in release. Saturate to zero.
        self.total_lines.saturating_sub(content_height)
      } else {
        new_offset as usize
      };

      // Move cursor to the match position
      self.cursor_y = line_idx.saturating_sub(self.offset);
      self.cursor_x = col_idx;
      self.cursor_moved = true;
    }
  }
}
