use super::core::Editor;
use crossterm::event::{self, Event as CEvent, KeyCode};

impl Editor {
  // Handle text object keys in visual mode (i/a prefix commands)
  pub fn handle_visual_text_object_keys(
    &mut self,
    key_code: KeyCode,
  ) -> Result<Option<bool>, Box<dyn std::error::Error>> {
    match key_code {
      KeyCode::Char('i') => {
        // Handle 'i' text objects (inner)
        let obj_key = if self.tutorial_demo_mode {
          // In demo mode, get the next key from pending keys
          if let Some(next_key) = self.check_demo_progress() {
            next_key
          } else {
            return Ok(Some(false));
          }
        } else {
          // Normal mode - read from keyboard
          match event::read()? {
            CEvent::Key(k) => k,
            _ => return Ok(Some(false)),
          }
        };
        match obj_key.code {
          KeyCode::Char('{') | KeyCode::Char('}') => {
            self.select_inner_braces();
          }
          KeyCode::Char('(') | KeyCode::Char(')') => {
            self.select_inner_parentheses();
          }
          KeyCode::Char('[') | KeyCode::Char(']') => {
            self.select_inner_brackets();
          }
          KeyCode::Char('"') => {
            self.select_inner_quotes('"');
          }
          KeyCode::Char('\'') => {
            self.select_inner_quotes('\'');
          }
          KeyCode::Char('p') => {
            self.select_inner_paragraph();
          }
          KeyCode::Char('s') => {
            self.select_inner_sentence();
          }
          KeyCode::Char('w') => {
            self.select_inner_word(false);
          }
          KeyCode::Char('W') => {
            self.select_inner_word(true);
          }
          _ => {}
        }
        Ok(Some(false))
      }
      KeyCode::Char('a') => {
        // Handle 'a' text objects (around)
        let obj_key = if self.tutorial_demo_mode {
          // In demo mode, get the next key from pending keys
          if let Some(next_key) = self.check_demo_progress() {
            next_key
          } else {
            return Ok(Some(false));
          }
        } else {
          // Normal mode - read from keyboard
          match event::read()? {
            CEvent::Key(k) => k,
            _ => return Ok(Some(false)),
          }
        };
        match obj_key.code {
          KeyCode::Char('{') | KeyCode::Char('}') => {
            self.select_around_braces();
          }
          KeyCode::Char('(') | KeyCode::Char(')') => {
            self.select_around_parentheses();
          }
          KeyCode::Char('[') | KeyCode::Char(']') => {
            self.select_around_brackets();
          }
          KeyCode::Char('"') => {
            self.select_around_quotes('"');
          }
          KeyCode::Char('\'') => {
            self.select_around_quotes('\'');
          }
          KeyCode::Char('p') => {
            self.select_around_paragraph();
          }
          KeyCode::Char('s') => {
            self.select_around_sentence();
          }
          KeyCode::Char('w') => {
            self.select_around_word(false);
          }
          KeyCode::Char('W') => {
            self.select_around_word(true);
          }
          _ => {}
        }
        Ok(Some(false))
      }
      _ => Ok(None),
    }
  }
}
