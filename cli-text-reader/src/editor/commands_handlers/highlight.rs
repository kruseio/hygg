use super::super::core::{Editor, EditorMode};

impl Editor {
  // Handle :h command - highlight selected text
  pub fn handle_highlight_command(
    &mut self,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    self.debug_log_event(
      "command",
      "highlight",
      "toggling highlight on selection",
    );
    self.toggle_highlight();
    // Track highlight creation for tutorial
    self.tutorial_highlight_created = true;
    self.set_active_mode(EditorMode::Normal);
    self.clear_selection(); // Clear selection after highlighting
    self.editor_state.command_buffer.clear();
    self.editor_state.command_cursor_pos = 0;
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.command_buffer.clear();
      buffer.command_cursor_pos = 0;
    }
    Ok(false)
  }

  // Handle :nohl command - clear search highlights
  pub fn handle_nohl_command(
    &mut self,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    self.debug_log_event("command", "nohlsearch", "clearing search highlights");
    self.editor_state.search_query.clear();
    self.editor_state.current_match = None;
    // Sync with active buffer
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.search_query.clear();
      buffer.current_match = None;
    }
    self.set_active_mode(EditorMode::Normal);
    self.editor_state.command_buffer.clear();
    self.editor_state.command_cursor_pos = 0;
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.command_buffer.clear();
      buffer.command_cursor_pos = 0;
    }
    self.mark_dirty();
    Ok(false)
  }
}
