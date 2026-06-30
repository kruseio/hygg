use super::super::core::{Editor, EditorMode};

impl Editor {
  // Handle :p command - toggle progress display
  pub fn handle_progress_command(
    &mut self,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    self.show_progress = !self.show_progress;
    self.save_current_config();
    self.set_active_mode(EditorMode::Normal);
    self.editor_state.command_buffer.clear();
    self.editor_state.command_cursor_pos = 0;
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.command_buffer.clear();
      buffer.command_cursor_pos = 0;
    }
    Ok(false)
  }

  // Handle :pagenumbers / :pn command - toggle page numbers in the status bar
  pub fn handle_page_numbers_command(
    &mut self,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    self.show_page_numbers = !self.show_page_numbers;
    self.save_current_config();
    self.set_active_mode(EditorMode::Normal);
    self.editor_state.command_buffer.clear();
    self.editor_state.command_cursor_pos = 0;
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.command_buffer.clear();
      buffer.command_cursor_pos = 0;
    }
    Ok(false)
  }

  // Handle :cursor command - toggle cursor visibility
  pub fn handle_cursor_command(
    &mut self,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    self.show_cursor = !self.show_cursor;
    self.save_current_config();
    self.set_active_mode(EditorMode::Normal);
    self.editor_state.command_buffer.clear();
    self.editor_state.command_cursor_pos = 0;
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.command_buffer.clear();
      buffer.command_cursor_pos = 0;
    }
    Ok(false)
  }
}
