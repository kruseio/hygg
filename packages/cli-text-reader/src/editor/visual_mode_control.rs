use super::core::{Editor, EditorMode};
use crossterm::event::{KeyCode, KeyModifiers};

impl Editor {
  // Handle visual mode control keys (esc, y, :, ctrl+c)
  pub fn handle_visual_control_keys(
    &mut self,
    key_code: KeyCode,
    modifiers: KeyModifiers,
  ) -> Result<Option<bool>, Box<dyn std::error::Error>> {
    // Handle Ctrl+C to exit to normal mode
    if key_code == KeyCode::Char('c')
      && modifiers.contains(KeyModifiers::CONTROL)
    {
      // Don't exit visual mode during tutorial
      if self.tutorial_active {
        return Ok(Some(false));
      }
      self.set_active_mode(EditorMode::Normal);
      self.clear_selection();
      return Ok(Some(false));
    }

    match key_code {
      KeyCode::Esc => {
        // Save selection before exiting visual mode
        self.save_visual_selection();
        // Exit visual mode
        self.set_active_mode(EditorMode::Normal);
        self.clear_selection();
        Ok(Some(false))
      }
      KeyCode::Char('y') => {
        // Yank selection and exit visual mode
        self.yank_selection();
        self.save_visual_selection();
        self.set_active_mode(EditorMode::Normal);
        self.clear_selection();
        Ok(Some(false))
      }
      KeyCode::Char(':') => {
        // Enter command mode from visual mode
        // Save the current visual mode before switching
        self.editor_state.previous_visual_mode = Some(self.get_active_mode());

        // Save the visual selection for both tutorial and normal mode
        self.save_visual_selection();

        // Mark that we have an active visual selection
        self.editor_state.visual_selection_active = true;

        self.set_active_mode(EditorMode::Command);
        // Don't clear selection here - we need it for commands like :h
        self.editor_state.command_buffer.clear();
        self.editor_state.command_cursor_pos = 0;
        if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
          buffer.command_buffer.clear();
          buffer.command_cursor_pos = 0;
        }
        Ok(Some(false))
      }
      _ => Ok(None),
    }
  }
}
