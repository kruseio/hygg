use super::{Editor, EditorMode};
use crate::progress::save_progress_full;

impl Editor {
  // Get the actual cursor position in the document (line_index, column)
  pub fn get_cursor_position(&self) -> (usize, usize) {
    // Calculate the correct line index based on the cursor's position
    // This ensures we get the line currently being displayed under the cursor
    let line_idx = self.offset + self.cursor_y;

    // Make sure we don't exceed the document boundaries
    let line_idx = line_idx.min(self.lines.len().saturating_sub(1));

    (line_idx, self.cursor_x)
  }

  // Debug logging helper
  pub fn debug_log(&self, message: &str) {
    crate::debug::debug_log("editor", message);
  }

  pub fn debug_log_event(&self, module: &str, event: &str, details: &str) {
    crate::debug::debug_log_event(module, event, details);
  }

  pub fn debug_log_state(
    &self,
    module: &str,
    state_name: &str,
    state_value: &str,
  ) {
    crate::debug::debug_log_state(module, state_name, state_value);
  }

  pub fn debug_log_error(&self, error: &str) {
    crate::debug::debug_log_error("editor", error);
  }

  // Calculate dimensions for display
  #[allow(dead_code)]
  pub fn calculate_dimensions(&self) -> usize {
    // Always use full height minus status line
    self.height.saturating_sub(1)
  }

  // Helper methods to access active buffer's mode and command state
  pub fn get_active_mode(&self) -> EditorMode {
    if let Some(buffer) = self.buffers.get(self.active_buffer) {
      buffer.mode.clone()
    } else {
      // Fallback to editor state mode during migration
      self.editor_state.mode.clone()
    }
  }

  pub fn set_active_mode(&mut self, mode: EditorMode) {
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.mode = mode.clone();
    }
    // Also update editor state during migration
    self.editor_state.mode = mode;
  }

  pub fn get_active_command_buffer(&self) -> &str {
    if let Some(buffer) = self.buffers.get(self.active_buffer) {
      &buffer.command_buffer
    } else {
      // Fallback to editor state during migration
      &self.editor_state.command_buffer
    }
  }

  #[allow(dead_code)]
  pub fn get_active_command_buffer_mut(&mut self) -> &mut String {
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      &mut buffer.command_buffer
    } else {
      // Fallback to editor state during migration
      &mut self.editor_state.command_buffer
    }
  }

  pub fn get_active_command_cursor_pos(&self) -> usize {
    if let Some(buffer) = self.buffers.get(self.active_buffer) {
      buffer.command_cursor_pos
    } else {
      // Fallback to editor state during migration
      self.editor_state.command_cursor_pos
    }
  }

  #[allow(dead_code)]
  pub fn set_active_command_cursor_pos(&mut self, pos: usize) {
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.command_cursor_pos = pos;
    }
    // Also update editor state during migration
    self.editor_state.command_cursor_pos = pos;
  }

  // Save bookmarks to file
  pub fn save_bookmarks(&self) {
    use crate::bookmarks::save_bookmarks;
    if let Err(e) = save_bookmarks(self.document_hash, &self.marks) {
      self.debug_log_error(&format!("Failed to save bookmarks: {e}"));
    }
  }

  // Save highlights to file
  pub fn save_highlights(&self) {
    use crate::highlights::save_highlights;
    if let Err(e) = save_highlights(&self.highlights) {
      self.debug_log_error(&format!("Failed to save highlights: {e}"));
    }
  }

  pub(crate) fn save_progress_snapshot(
    &mut self,
    force: bool,
  ) -> Result<(), Box<dyn std::error::Error>> {
    if self.total_lines == 0 {
      return Ok(());
    }
    if self.pdf_pending.is_some() && self.pdf_streaming.is_none() {
      return Ok(());
    }

    let current_line = self.offset + self.cursor_y;
    if !force
      && current_line == self.last_offset
      && self.offset == self.last_saved_viewport_offset
    {
      return Ok(());
    }

    let (page, line_in_page) = match self.current_pdf_position() {
      Some((p, l)) => (Some(p), Some(l)),
      None => (None, None),
    };
    save_progress_full(
      self.document_hash,
      current_line,
      self.total_lines,
      Some(self.offset),
      Some(self.cursor_y),
      page,
      line_in_page,
    )?;
    self.last_offset = current_line;
    self.last_saved_viewport_offset = self.offset;
    Ok(())
  }

  // Mark editor as needing redraw
  pub fn mark_dirty(&mut self) {
    self.needs_redraw = true;
  }

  // Check if redraw is needed and reset flag
  pub fn check_needs_redraw(&mut self) -> bool {
    let needs = self.needs_redraw;
    self.needs_redraw = false;
    needs
  }

  // Get the effective viewport height for the current buffer
  pub fn get_effective_viewport_height(&self) -> usize {
    if self.view_mode == super::ViewMode::HorizontalSplit {
      if let Some(buffer) = self.buffers.get(self.active_buffer) {
        buffer.viewport_height
      } else {
        self.height.saturating_sub(1)
      }
    } else {
      self.height.saturating_sub(1)
    }
  }
}
