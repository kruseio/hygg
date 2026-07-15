use super::core::Editor;
use crossterm::event::{KeyCode, KeyModifiers};

impl Editor {
  // Handle movement keys in visual mode with selection updates
  pub fn handle_visual_movement_keys(
    &mut self,
    key_code: KeyCode,
    modifiers: KeyModifiers,
  ) -> Result<Option<bool>, Box<dyn std::error::Error>> {
    match key_code {
      KeyCode::Char('j') | KeyCode::Down => {
        self.move_cursor_down();
        self.update_selection();
        Ok(Some(false))
      }
      KeyCode::Char('k') | KeyCode::Up => {
        self.move_cursor_up();
        self.update_selection();
        Ok(Some(false))
      }
      KeyCode::Char('h') | KeyCode::Left => {
        self.move_cursor_left();
        self.update_selection();
        Ok(Some(false))
      }
      KeyCode::Char('l') | KeyCode::Right => {
        self.move_cursor_right();
        self.update_selection();
        Ok(Some(false))
      }
      KeyCode::Char('w') => {
        // Word forward motion
        self.move_to_word_boundary(true, false);
        self.update_selection();
        Ok(Some(false))
      }
      KeyCode::Char('W') => {
        // Big word forward motion
        self.move_to_word_boundary(true, true);
        self.update_selection();
        Ok(Some(false))
      }
      KeyCode::Char('b') => {
        // Word backward motion
        self.move_to_word_boundary(false, false);
        self.update_selection();
        Ok(Some(false))
      }
      KeyCode::Char('B') => {
        // Big word backward motion
        self.move_to_word_boundary(false, true);
        self.update_selection();
        Ok(Some(false))
      }
      KeyCode::Char('e') => {
        // End of word motion in visual mode
        let (new_line, new_col) = self.find_word_end(false);
        self.move_to_position(new_line, new_col);
        self.update_selection();
        Ok(Some(false))
      }
      KeyCode::Char('E') => {
        // End of big word motion in visual mode
        let (new_line, new_col) = self.find_word_end(true);
        self.move_to_position(new_line, new_col);
        self.update_selection();
        Ok(Some(false))
      }
      KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
        // Ctrl+U - half page up in visual mode
        self.half_page_up();
        self.update_selection();
        Ok(Some(false))
      }
      KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => {
        // Ctrl+D - half page down in visual mode
        self.half_page_down();
        self.update_selection();
        Ok(Some(false))
      }
      KeyCode::Char('0') => {
        // '0' - jump to start of line in visual mode
        self.cursor_x = 0;
        self.update_selection();
        Ok(Some(false))
      }
      KeyCode::Char('$') => {
        // '$' - jump to end of line in visual mode
        let current_line_idx = self.offset + self.cursor_y;
        if current_line_idx < self.lines.len() {
          let line_length = self.lines[current_line_idx].len();
          self.cursor_x = if line_length > 0 { line_length - 1 } else { 0 };
        }
        self.update_selection();
        Ok(Some(false))
      }
      KeyCode::Char('^') => {
        // '^' - jump to first non-whitespace character in visual mode
        let current_line_idx = self.offset + self.cursor_y;
        if current_line_idx < self.lines.len() {
          let line = &self.lines[current_line_idx];
          for (idx, c) in line.char_indices() {
            if !c.is_whitespace() {
              self.cursor_x = idx;
              self.update_selection();
              return Ok(Some(false));
            }
          }
          // If line is all whitespace, go to start
          self.cursor_x = 0;
          self.update_selection();
        }
        Ok(Some(false))
      }
      KeyCode::Char('%') => {
        // Jump to matching bracket/parenthesis in visual mode
        if let Some((line, col)) = self.find_matching_bracket() {
          self.move_to_position(line, col);
          self.update_selection();
        }
        Ok(Some(false))
      }
      _ => Ok(None),
    }
  }
}
