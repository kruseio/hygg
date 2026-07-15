use crossterm::event;
use std::io;

use super::core::{Editor, EditorMode};

impl Editor {
  // Main event dispatcher - routes events to appropriate mode handlers
  pub fn handle_event(
    &mut self,
    key_event: event::KeyEvent,
    stdout: &mut io::Stdout,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    let active_mode = self.get_active_mode();
    self.debug_log(&format!(
      "=== handle_event: key={:?}, active_buffer={}, active_mode={:?}, view_mode={:?} ===",
      key_event, self.active_buffer, active_mode, self.view_mode
    ));

    // Route to mode-specific handlers first
    let result = match active_mode {
      EditorMode::Normal => self.handle_normal_mode_event(key_event),
      EditorMode::VisualChar | EditorMode::VisualLine => {
        self.handle_visual_mode_event(key_event)
      }
      EditorMode::Search | EditorMode::ReverseSearch => {
        self.handle_search_mode_event(key_event)
      }
      EditorMode::Command | EditorMode::CommandExecution => {
        self.handle_command_mode_event(key_event, stdout)
      }
      EditorMode::Tutorial => Ok(self.process_tutorial_key(key_event.code)),
    };

    // Process tutorial key after mode-specific handling, only if in tutorial
    // mode
    if self.tutorial_active {
      // Don't process tutorial keys if we're in a special input mode
      match self.get_active_mode() {
        EditorMode::Command
        | EditorMode::CommandExecution
        | EditorMode::Search
        | EditorMode::ReverseSearch => {
          // Skip tutorial processing for input modes
        }
        _ => {
          // Process tutorial key for other modes
          self.process_tutorial_key(key_event.code);
        }
      }
    }

    result
  }
}
