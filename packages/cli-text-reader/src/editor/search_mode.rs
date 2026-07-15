use crossterm::event::{self, KeyCode, KeyModifiers};

use super::core::{Editor, EditorMode};

impl Editor {
  // Handle key events in search modes (forward and reverse)
  pub fn handle_search_mode_event(
    &mut self,
    key_event: event::KeyEvent,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    // Handle Ctrl+C to exit to normal mode
    if key_event.code == KeyCode::Char('c')
      && key_event.modifiers.contains(KeyModifiers::CONTROL)
    {
      self.set_active_mode(EditorMode::Normal);
      self.editor_state.command_buffer.clear();
      if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
        buffer.command_buffer.clear();
        buffer.command_cursor_pos = 0;
      }
      self.mark_dirty();
      return Ok(false);
    }

    match key_event.code {
      KeyCode::Esc => {
        // Restore original cursor position
        if let (Some((y, x)), Some(offset)) = (
          self.editor_state.search_original_cursor,
          self.editor_state.search_original_offset,
        ) {
          self.cursor_y = y;
          self.cursor_x = x;
          self.offset = offset;
          self.cursor_moved = true;
        }
        self.editor_state.search_preview_active = false;
        self.editor_state.search_preview_match = None;
        self.set_active_mode(EditorMode::Normal);
        self.editor_state.command_buffer.clear();
        if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
          buffer.command_buffer.clear();
          buffer.command_cursor_pos = 0;
        }
        self.mark_dirty();
      }
      KeyCode::Enter => {
        self.editor_state.search_query =
          self.editor_state.command_buffer.clone();
        // Move cursor to the preview match if found
        if let Some((_line, _col, _)) = self.editor_state.search_preview_match {
          // Set current_match from preview
          self.editor_state.current_match =
            self.editor_state.search_preview_match;
          // Also update active buffer's current_match
          if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
            buffer.current_match = self.editor_state.search_preview_match;
          }
          // Move cursor to match
          self.center_on_match();
        } else {
          // No match found, just exit search mode without moving
        }
        self.editor_state.search_preview_active = false;
        self.editor_state.search_preview_match = None;
        self.set_active_mode(EditorMode::Normal);
        self.editor_state.command_buffer.clear();
        if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
          buffer.command_buffer.clear();
          buffer.command_cursor_pos = 0;
        }
        self.mark_dirty();
      }
      KeyCode::Backspace => {
        if !self.editor_state.command_buffer.is_empty() {
          self.editor_state.command_buffer.pop();
          self.editor_state.command_cursor_pos =
            self.editor_state.command_buffer.len();

          // Sync with active buffer
          if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
            buffer.command_buffer = self.editor_state.command_buffer.clone();
            buffer.command_cursor_pos = self.editor_state.command_buffer.len();
          }
          // Update preview match
          let query = self.editor_state.command_buffer.clone();
          let direction = self.editor_state.search_direction;
          self.find_preview_match(&query, direction);
        } else {
          // If search buffer is already empty, exit search mode and restore
          // cursor
          if let (Some((y, x)), Some(offset)) = (
            self.editor_state.search_original_cursor,
            self.editor_state.search_original_offset,
          ) {
            self.cursor_y = y;
            self.cursor_x = x;
            self.offset = offset;
            self.cursor_moved = true;
          }
          self.editor_state.search_preview_active = false;
          self.editor_state.search_preview_match = None;
          self.set_active_mode(EditorMode::Normal);
          if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
            buffer.command_buffer.clear();
            buffer.command_cursor_pos = 0;
          }
        }
        self.mark_dirty();
      }
      KeyCode::Char(c) => {
        self.editor_state.command_buffer.push(c);
        // Sync with active buffer
        if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
          buffer.command_buffer = self.editor_state.command_buffer.clone();
          buffer.command_cursor_pos = self.editor_state.command_buffer.len();
        }
        // Update preview match for real-time highlighting
        let query = self.editor_state.command_buffer.clone();
        let direction = self.editor_state.search_direction;
        self.find_preview_match(&query, direction);
        self.mark_dirty();
      }
      _ => {}
    }
    Ok(false)
  }
}
