use crossterm::event::{self, Event as CEvent, KeyCode, KeyModifiers};

use super::core::{Editor, EditorMode};

impl Editor {
  // Handle search and visual mode related key events in normal mode
  pub fn handle_search_visual_keys(
    &mut self,
    key_event: event::KeyEvent,
  ) -> Result<Option<bool>, Box<dyn std::error::Error>> {
    match key_event.code {
      KeyCode::Char('/') => {
        self.set_active_mode(EditorMode::Search);
        self.editor_state.command_buffer.clear();
        self.editor_state.command_cursor_pos = 0;
        // Sync with active buffer
        if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
          buffer.command_buffer.clear();
          buffer.command_cursor_pos = 0;
        }
        self.editor_state.search_direction = true;
        // Save current cursor position for preview mode
        self.editor_state.search_preview_active = true;
        self.editor_state.search_original_cursor =
          Some((self.cursor_y, self.cursor_x));
        self.editor_state.search_original_offset = Some(self.offset);
        self.editor_state.search_preview_match = None;
        // Track for tutorial
        if self.tutorial_active {
          self.tutorial_forward_search_used = true;
        }
        self.mark_dirty();
        Ok(Some(false))
      }
      KeyCode::Char('?') => {
        self.set_active_mode(EditorMode::ReverseSearch);
        self.editor_state.command_buffer.clear();
        self.editor_state.command_cursor_pos = 0;
        // Sync with active buffer
        if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
          buffer.command_buffer.clear();
          buffer.command_cursor_pos = 0;
        }
        self.editor_state.search_direction = false;
        // Save current cursor position for preview mode
        self.editor_state.search_preview_active = true;
        self.editor_state.search_original_cursor =
          Some((self.cursor_y, self.cursor_x));
        self.editor_state.search_original_offset = Some(self.offset);
        self.editor_state.search_preview_match = None;
        // Track for tutorial
        if self.tutorial_active {
          self.tutorial_backward_search_used = true;
        }
        self.mark_dirty();
        Ok(Some(false))
      }
      KeyCode::Char('n') => {
        if !self.editor_state.search_query.is_empty() {
          // Use the original search direction
          self.find_next_match(self.editor_state.search_direction);
          self.center_on_match();
          // Track navigation for tutorial
          if self.tutorial_active {
            self.tutorial_search_navigated = true;
          }
        }
        Ok(Some(false))
      }
      KeyCode::Char('N') => {
        if !self.editor_state.search_query.is_empty() {
          // Use opposite of original search direction
          self.find_next_match(!self.editor_state.search_direction);
          self.center_on_match();
          // Track navigation for tutorial
          if self.tutorial_active {
            self.tutorial_search_navigated = true;
          }
        }
        Ok(Some(false))
      }
      KeyCode::Char('*') => {
        // Search for word under cursor forward
        if let Some(word) = self.get_word_under_cursor() {
          self.editor_state.search_query = word;
          self.editor_state.search_direction = true;
          self.find_next_match(true);
          self.center_on_match();
          // Track search for tutorial
          if self.tutorial_active {
            // Check if this completes the search tutorial step
            if !self.tutorial_step_completed && self.check_tutorial_completion()
            {
              self.tutorial_step_completed = true;
              self.update_tutorial_step();
            }
          }
        }
        Ok(Some(false))
      }
      KeyCode::Char('#') => {
        // Search for word under cursor backward
        if let Some(word) = self.get_word_under_cursor() {
          self.editor_state.search_query = word;
          self.editor_state.search_direction = false;
          self.find_next_match(false);
          self.center_on_match();
        }
        Ok(Some(false))
      }
      KeyCode::Char('v') => {
        // Enter visual character mode
        self.set_active_mode(EditorMode::VisualChar);
        self.start_selection();
        Ok(Some(false))
      }
      KeyCode::Char('V') => {
        // Enter visual line mode
        self.set_active_mode(EditorMode::VisualLine);
        self.start_selection();
        Ok(Some(false))
      }
      KeyCode::Char('y') => {
        self.debug_log_event(
          "normal_mode",
          "y_pressed",
          &format!(
            "modifiers={:?}, operator_pending={:?}",
            key_event.modifiers, self.editor_state.operator_pending
          ),
        );

        // Check if we already have 'y' operator pending (for 'yy' command)
        if let Some('y') = self.editor_state.operator_pending {
          // Second 'y' press - execute yank line
          self.debug_log_event(
            "normal_mode",
            "yy_detected",
            "executing yank_line",
          );
          self.yank_line();
          self.editor_state.operator_pending = None;
        } else {
          match key_event.modifiers {
            event::KeyModifiers::SHIFT => {
              // 'Y' - Same as 'yy'
              self.debug_log_event(
                "normal_mode",
                "shift_y_yank",
                "executing yank_line",
              );
              self.yank_line();
            }
            _ => {
              // Set operator pending for 'y'
              self.debug_log_event(
                "normal_mode",
                "y_operator_pending",
                "setting operator_pending = y",
              );
              self.editor_state.operator_pending = Some('y');
            }
          }
        }
        Ok(Some(false))
      }
      KeyCode::Char('i') => {
        // Text object selection
        if let Some('v') = self.editor_state.operator_pending {
          // For visual text object selection operations like 'viw'
          let inner_key = if self.tutorial_demo_mode {
            if let Some(next_key) = self.check_demo_progress() {
              next_key
            } else {
              return Ok(Some(false));
            }
          } else {
            match event::read()? {
              CEvent::Key(k) => k,
              _ => return Ok(Some(false)),
            }
          };
          match inner_key.code {
            KeyCode::Char('w') => {
              // Inner word
              self.select_inner_word(false);
              self.editor_state.operator_pending = None;
            }
            KeyCode::Char('W') => {
              // Inner WORD
              self.select_inner_word(true);
              self.editor_state.operator_pending = None;
            }
            KeyCode::Char('"')
            | KeyCode::Char('\'')
            | KeyCode::Char('(')
            | KeyCode::Char(')')
            | KeyCode::Char('{')
            | KeyCode::Char('}')
            | KeyCode::Char('[')
            | KeyCode::Char(']') => {
              // Inner quotes, parentheses, braces, brackets
              if let KeyCode::Char(c) = inner_key.code
                && let Some((start, end)) = self.find_text_object(c)
              {
                let line_idx = self.offset + self.cursor_y;
                self.editor_state.selection_start = Some((line_idx, start));
                self.editor_state.selection_end = Some((line_idx, end));
              }
              self.editor_state.operator_pending = None;
            }
            _ => {
              self.editor_state.operator_pending = None;
            }
          }
        }
        Ok(Some(false))
      }
      _ => Ok(None), // Not handled by search/visual
    }
  }

  // Handle operator pending operations (like 'y' followed by motion)
  pub fn handle_operator_pending(
    &mut self,
    key_event: event::KeyEvent,
  ) -> Result<Option<bool>, Box<dyn std::error::Error>> {
    if let Some(op) = self.editor_state.operator_pending {
      self.debug_log_event(
        "normal_mode",
        "operator_pending",
        &format!("op={}, key={:?}", op, key_event.code),
      );
      match op {
        'y' => {
          match key_event.code {
            KeyCode::Char('y') => {
              // 'yy' - Yank current line
              self.debug_log_event(
                "normal_mode",
                "yank_line_execute",
                &format!("cursor_line={}", self.offset + self.cursor_y),
              );
              self.yank_line();
            }
            KeyCode::Char('w') => {
              // 'yw' - Yank word
              self.debug_log_event("normal_mode", "yank_word_execute", "");
              self.yank_word();
            }
            _ => {
              self.debug_log_event(
                "normal_mode",
                "operator_cancelled",
                &format!("invalid key for y: {:?}", key_event.code),
              );
            }
          }
          self.editor_state.operator_pending = None;
          self.debug_log_event("normal_mode", "operator_pending_cleared", "");
          Ok(Some(false))
        }
        _ => {
          self.debug_log_event(
            "normal_mode",
            "unknown_operator",
            &format!("op={op}"),
          );
          self.editor_state.operator_pending = None;
          Ok(Some(false))
        }
      }
    } else {
      Ok(None)
    }
  }
}
