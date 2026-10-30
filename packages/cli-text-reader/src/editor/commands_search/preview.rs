use super::super::char_boundary::{ByteCol, CursorCol, LineSpan};
use super::super::core::Editor;
use super::matching::{find_ci, find_ci_from, rfind_ci_before};

impl Editor {
  // Find match for preview and move cursor to preview position
  pub fn find_preview_match(&mut self, query: &str, forward: bool) {
    if query.is_empty() {
      self.editor_state.search_preview_match = None;
      return;
    }

    // Use original saved position for search, not current cursor position
    let (search_line, search_x) = if let (Some((y, x)), Some(offset)) = (
      self.editor_state.search_original_cursor,
      self.editor_state.search_original_offset,
    ) {
      (offset + y, x)
    } else {
      (self.offset + self.cursor_y, self.cursor_x)
    };

    if forward {
      // First check current line from cursor position onward
      if search_line < self.lines.len() && !self.is_ansi_art_line(search_line) {
        let line = &self.lines[search_line];
        if search_x.col() < ByteCol::end(line)
          && let Some((start, end)) = find_ci_from(line, query, search_x.col())
        {
          self.editor_state.search_preview_match =
            Some(LineSpan::new(search_line, start, end));
          // Also store in active buffer for split view
          if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
            buffer.current_match = Some(LineSpan::new(search_line, start, end));
          }
          self.center_on_preview_match();
          return;
        }
      }

      // Then search forward from next line
      for i in search_line + 1..self.lines.len() {
        if self.is_ansi_art_line(i) {
          continue;
        }
        if let Some((start, end)) = find_ci(&self.lines[i], query) {
          self.editor_state.search_preview_match =
            Some(LineSpan::new(i, start, end));
          // Also store in active buffer for split view
          if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
            buffer.current_match = Some(LineSpan::new(i, start, end));
          }
          self.center_on_preview_match();
          return;
        }
      }
      // Wrap around to beginning
      for i in 0..=search_line {
        if self.is_ansi_art_line(i) {
          continue;
        }
        if let Some((start, end)) = find_ci(&self.lines[i], query) {
          self.editor_state.search_preview_match =
            Some(LineSpan::new(i, start, end));
          // Also store in active buffer for split view
          if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
            buffer.current_match = Some(LineSpan::new(i, start, end));
          }
          self.center_on_preview_match();
          return;
        }
      }
    } else {
      // Backward search logic
      if search_line < self.lines.len()
        && search_x > CursorCol::ZERO
        && !self.is_ansi_art_line(search_line)
      {
        let line = &self.lines[search_line];
        if let Some((start, end)) = rfind_ci_before(line, query, search_x.col())
        {
          self.editor_state.search_preview_match =
            Some(LineSpan::new(search_line, start, end));
          // Also store in active buffer for split view
          if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
            buffer.current_match = Some(LineSpan::new(search_line, start, end));
          }
          self.center_on_preview_match();
          return;
        }
      }

      // Then search backward from previous line
      for i in (0..search_line).rev() {
        if self.is_ansi_art_line(i) {
          continue;
        }
        if let Some((start, end)) = find_ci(&self.lines[i], query) {
          self.editor_state.search_preview_match =
            Some(LineSpan::new(i, start, end));
          // Also store in active buffer for split view
          if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
            buffer.current_match = Some(LineSpan::new(i, start, end));
          }
          self.center_on_preview_match();
          return;
        }
      }
      // Wrap around to end
      for i in (search_line..self.lines.len()).rev() {
        if self.is_ansi_art_line(i) {
          continue;
        }
        if let Some((start, end)) = find_ci(&self.lines[i], query) {
          self.editor_state.search_preview_match =
            Some(LineSpan::new(i, start, end));
          // Also store in active buffer for split view
          if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
            buffer.current_match = Some(LineSpan::new(i, start, end));
          }
          self.center_on_preview_match();
          return;
        }
      }
    }

    // No match found - restore original position
    self.editor_state.search_preview_match = None;
    // Clear match in active buffer
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.current_match = None;
    }
    if let (Some((y, x)), Some(offset)) = (
      self.editor_state.search_original_cursor,
      self.editor_state.search_original_offset,
    ) {
      self.offset = offset;
      self.cursor_y = y;
      self.cursor_x = x;
      self.cursor_moved = true;
    }
  }

  // Center the view on the preview match and move cursor to preview it
  pub fn center_on_preview_match(&mut self) {
    if let Some(LineSpan { line: line_idx, start: col_idx, .. }) =
      self.editor_state.search_preview_match
    {
      // Center the view
      let content_height = self.height.saturating_sub(1);
      let half_height = (content_height / 2) as i32;
      let new_offset = line_idx as i32 - half_height;
      self.offset = if new_offset < 0 {
        0
      } else if new_offset + content_height as i32 > self.total_lines as i32 {
        self.total_lines.saturating_sub(content_height)
      } else {
        new_offset as usize
      };

      // Move cursor to the match position for preview (floored in case the
      // line changed since the match was found)
      self.cursor_y = line_idx.saturating_sub(self.offset);
      self.cursor_x = self
        .lines
        .get(line_idx)
        .map_or(CursorCol::ZERO, |line| CursorCol::floor(line, col_idx));
      self.cursor_moved = true;
    }
  }
}
