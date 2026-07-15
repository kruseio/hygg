use super::core::Editor;
use crossterm::event;

impl Editor {
  // Handle navigation-related key events in normal mode
  pub fn handle_navigation_keys(
    &mut self,
    key_event: event::KeyEvent,
  ) -> Result<Option<bool>, Box<dyn std::error::Error>> {
    let result = self.dispatch_navigation_keys(key_event)?;
    // A deliberate move while the server-progress prompt is up = "keep my
    // place": start the grace after which the local position overrides the
    // server's (see `tick_server_progress_grace`).
    if result.is_some() {
      self.note_user_scrolled();
    }
    Ok(result)
  }

  fn dispatch_navigation_keys(
    &mut self,
    key_event: event::KeyEvent,
  ) -> Result<Option<bool>, Box<dyn std::error::Error>> {
    // Try basic movement keys first
    if let Ok(Some(result)) = self.handle_basic_movement_keys(key_event.code) {
      return Ok(Some(result));
    }

    // Try word movement keys
    if let Ok(Some(result)) = self.handle_word_movement_keys(key_event.code) {
      return Ok(Some(result));
    }

    // Try character finding keys
    if let Ok(Some(result)) = self.handle_char_find_keys(key_event.code) {
      return Ok(Some(result));
    }

    // Try page movement keys
    if let Ok(Some(result)) =
      self.handle_page_movement_keys(key_event.code, key_event.modifiers)
    {
      return Ok(Some(result));
    }

    // Try jump keys
    if let Ok(Some(result)) = self.handle_jump_keys(key_event.code) {
      return Ok(Some(result));
    }

    // Try text object keys
    if let Ok(Some(result)) = self.handle_text_object_keys(key_event.code) {
      return Ok(Some(result));
    }

    // Try mark keys
    if let Ok(Some(result)) = self.handle_mark_keys(key_event.code) {
      return Ok(Some(result));
    }

    // Not handled by navigation
    Ok(None)
  }
}
