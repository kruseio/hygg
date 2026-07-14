use crossterm::event;

use super::core::Editor;

impl Editor {
  // Handle key events in normal mode - dispatcher to specialized handlers
  pub fn handle_normal_mode_event(
    &mut self,
    key_event: event::KeyEvent,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    self.debug_log_event("normal_mode", "key_press", &format!("{key_event:?}"));

    // The notes overlay captures every keystroke as note text before any
    // vim-style handling runs, so typing notes never moves the cursor or
    // triggers commands in the underlying document.
    if self.notes_active {
      return self.handle_notes_input_key(key_event);
    }

    // Handle number prefix clearing first
    self.handle_number_prefix_clearing(key_event);

    // Handle tmux prefix mode if active
    if self.tmux_prefix_active
      && let Some(result) = self.handle_tmux_prefix(key_event)?
    {
      return Ok(result);
    }

    // Try control keys first (mode switching, etc.)
    if let Some(result) = self.handle_control_keys(key_event)? {
      return Ok(result);
    }

    // Try operator pending operations
    if let Some(result) = self.handle_operator_pending(key_event)? {
      return Ok(result);
    }

    // Try search and visual mode operations
    if let Some(result) = self.handle_search_visual_keys(key_event)? {
      return Ok(result);
    }

    // Try navigation operations
    if let Some(result) = self.handle_navigation_keys(key_event)? {
      return Ok(result);
    }

    // If no handler claimed the event, it's unhandled
    Ok(false)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

  #[test]
  fn y_does_not_jump_to_pending_server_progress() {
    let mut editor = Editor::new(vec!["line".to_string(); 100], 80);
    editor.offset = 10;
    editor.cursor_y = 0;
    editor.server_progress_prompt = true;
    editor.pending_server_progress = Some(crate::sync::ServerProgress {
      book_id: "doc".to_string(),
      offset: 75,
      total_lines: 0,
      percentage: 0.0,
      viewport_offset: None,
      cursor_y: None,
      page: None,
      line_in_page: None,
      word_offset: None,
      updated_at: 2_000,
    });

    editor
      .handle_normal_mode_event(KeyEvent::new(
        KeyCode::Char('y'),
        KeyModifiers::empty(),
      ))
      .unwrap();

    assert_eq!(editor.offset + editor.cursor_y, 10);
    assert!(editor.server_progress_prompt);
    assert_eq!(
      editor.pending_server_progress.as_ref().map(|p| p.offset),
      Some(75)
    );
  }
}
