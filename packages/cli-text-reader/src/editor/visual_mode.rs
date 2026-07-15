use super::core::Editor;
use crossterm::event;

impl Editor {
  // Handle key events in visual mode (char and line)
  pub fn handle_visual_mode_event(
    &mut self,
    key_event: event::KeyEvent,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    // Try visual control keys first
    if let Ok(Some(_)) =
      self.handle_visual_control_keys(key_event.code, key_event.modifiers)
    {
      return Ok(false);
    }

    // Try movement keys
    if let Ok(Some(_)) =
      self.handle_visual_movement_keys(key_event.code, key_event.modifiers)
    {
      return Ok(false);
    }

    // Try character finding keys
    if let Ok(Some(_)) = self.handle_visual_find_keys(key_event.code) {
      return Ok(false);
    }

    // Try text object keys
    if let Ok(Some(_)) = self.handle_visual_text_object_keys(key_event.code) {
      return Ok(false);
    }

    // Not handled
    Ok(false)
  }
}
