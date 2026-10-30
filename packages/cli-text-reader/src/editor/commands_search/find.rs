use super::super::char_boundary::{ByteCol, CursorCol, LineSpan};
use super::super::core::Editor;
use super::matching::{find_ci, find_ci_from, rfind_ci, rfind_ci_before};

impl Editor {
  // Find first match from current cursor position (for new searches)
  #[allow(dead_code)]
  pub fn find_first_match(&mut self, forward: bool) {
    if self.editor_state.search_query.is_empty() {
      return;
    }

    let query = self.editor_state.search_query.clone();
    let current_line = self.offset + self.cursor_y;

    // Clear any existing match to ensure we search from cursor position
    self.editor_state.current_match = None;

    if forward {
      // First check current line from cursor position onward
      if current_line < self.lines.len() && !self.is_ansi_art_line(current_line)
      {
        let line = &self.lines[current_line];
        if self.cursor_x.col() < ByteCol::end(line)
          && let Some((start, end)) =
            find_ci_from(line, &query, self.cursor_x.col())
        {
          self.editor_state.current_match =
            Some(LineSpan::new(current_line, start, end));
          return;
        }
      }

      // Then search forward from next line
      for i in current_line + 1..self.lines.len() {
        if self.is_ansi_art_line(i) {
          continue;
        }
        if let Some((start, end)) = find_ci(&self.lines[i], &query) {
          self.editor_state.current_match = Some(LineSpan::new(i, start, end));
          // Also update active buffer's current_match
          if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
            buffer.current_match = Some(LineSpan::new(i, start, end));
          }
          return;
        }
      }
      // Wrap around to beginning
      for i in 0..=current_line {
        if self.is_ansi_art_line(i) {
          continue;
        }
        if let Some((start, end)) = find_ci(&self.lines[i], &query) {
          self.editor_state.current_match = Some(LineSpan::new(i, start, end));
          // Also update active buffer's current_match
          if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
            buffer.current_match = Some(LineSpan::new(i, start, end));
          }
          return;
        }
      }
    } else {
      // First check current line from cursor position backward
      if current_line < self.lines.len()
        && self.cursor_x > CursorCol::ZERO
        && !self.is_ansi_art_line(current_line)
      {
        let line = &self.lines[current_line];
        if let Some((start, end)) =
          rfind_ci_before(line, &query, self.cursor_x.col())
        {
          self.editor_state.current_match =
            Some(LineSpan::new(current_line, start, end));
          return;
        }
      }

      // Then search backward from previous line
      for i in (0..current_line).rev() {
        if self.is_ansi_art_line(i) {
          continue;
        }
        if let Some((start, end)) = find_ci(&self.lines[i], &query) {
          self.editor_state.current_match = Some(LineSpan::new(i, start, end));
          // Also update active buffer's current_match
          if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
            buffer.current_match = Some(LineSpan::new(i, start, end));
          }
          return;
        }
      }
      // Wrap around to end
      for i in (current_line..self.lines.len()).rev() {
        if self.is_ansi_art_line(i) {
          continue;
        }
        if let Some((start, end)) = find_ci(&self.lines[i], &query) {
          self.editor_state.current_match = Some(LineSpan::new(i, start, end));
          // Also update active buffer's current_match
          if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
            buffer.current_match = Some(LineSpan::new(i, start, end));
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

    let query = self.editor_state.search_query.clone();
    let start_idx = if let Some(found) = self.editor_state.current_match {
      found.line
    } else {
      self.offset + self.cursor_y
    };

    if forward {
      // Forward search
      for i in start_idx + 1..self.lines.len() {
        if self.is_ansi_art_line(i) {
          continue;
        }
        if let Some((start, end)) = find_ci(&self.lines[i], &query) {
          self.editor_state.current_match = Some(LineSpan::new(i, start, end));
          // Also update active buffer's current_match
          if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
            buffer.current_match = Some(LineSpan::new(i, start, end));
          }
          return;
        }
      }
      // Wrap around to beginning
      for i in 0..=start_idx {
        if self.is_ansi_art_line(i) {
          continue;
        }
        if let Some((start, end)) = find_ci(&self.lines[i], &query) {
          self.editor_state.current_match = Some(LineSpan::new(i, start, end));
          // Also update active buffer's current_match
          if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
            buffer.current_match = Some(LineSpan::new(i, start, end));
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
        if let Some((start, end)) = rfind_ci(&self.lines[i], &query) {
          self.editor_state.current_match = Some(LineSpan::new(i, start, end));
          // Also update active buffer's current_match
          if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
            buffer.current_match = Some(LineSpan::new(i, start, end));
          }
          return;
        }
      }
      // Wrap around to end
      for i in (start_idx..self.lines.len()).rev() {
        if self.is_ansi_art_line(i) {
          continue;
        }
        if let Some((start, end)) = rfind_ci(&self.lines[i], &query) {
          self.editor_state.current_match = Some(LineSpan::new(i, start, end));
          // Also update active buffer's current_match
          if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
            buffer.current_match = Some(LineSpan::new(i, start, end));
          }
          return;
        }
      }
    }
  }

  // Center the view on the current search match and move cursor to it
  pub fn center_on_match(&mut self) {
    if let Some(LineSpan { line: line_idx, start: col_idx, .. }) =
      self.editor_state.current_match
    {
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

      // Move cursor to the match position. Match columns are byte offsets on
      // character boundaries; floor anyway in case the line changed since.
      self.cursor_y = line_idx.saturating_sub(self.offset);
      self.cursor_x = self
        .lines
        .get(line_idx)
        .map_or(CursorCol::ZERO, |line| CursorCol::floor(line, col_idx));
      self.cursor_moved = true;
    }
  }
}
