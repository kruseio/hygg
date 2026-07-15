use super::core::Editor;
use crossterm::event::{self, Event as CEvent, KeyCode};

impl Editor {
  // Handle character finding keys in visual mode (f/F/t/T)
  pub fn handle_visual_find_keys(
    &mut self,
    key_code: KeyCode,
  ) -> Result<Option<bool>, Box<dyn std::error::Error>> {
    match key_code {
      KeyCode::Char('f') => {
        // Find character forward on line
        let char_key = if self.tutorial_demo_mode {
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
        if let KeyCode::Char(c) = char_key.code
          && let Some(pos) = self.find_char_on_line(c, true, false)
        {
          self.cursor_x = pos;
          self.update_selection();
        }
        Ok(Some(false))
      }
      KeyCode::Char('F') => {
        // Find character backward on line
        let char_key = if self.tutorial_demo_mode {
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
        if let KeyCode::Char(c) = char_key.code
          && let Some(pos) = self.find_char_on_line(c, false, false)
        {
          self.cursor_x = pos;
          self.update_selection();
        }
        Ok(Some(false))
      }
      KeyCode::Char('t') => {
        // Till character forward on line (stop before)
        let char_key = if self.tutorial_demo_mode {
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
        if let KeyCode::Char(c) = char_key.code
          && let Some(pos) = self.find_char_on_line(c, true, true)
        {
          self.cursor_x = pos;
          self.update_selection();
        }
        Ok(Some(false))
      }
      KeyCode::Char('T') => {
        // Till character backward on line (stop after)
        let char_key = if self.tutorial_demo_mode {
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
        if let KeyCode::Char(c) = char_key.code
          && let Some(pos) = self.find_char_on_line(c, false, true)
        {
          self.cursor_x = pos;
          self.update_selection();
        }
        Ok(Some(false))
      }
      _ => Ok(None),
    }
  }
}
