use super::core::Editor;
use crossterm::event::{self, Event as CEvent, KeyCode};

impl Editor {
  // Handle jump/goto keys (g/G/0/$^/%)
  pub fn handle_jump_keys(
    &mut self,
    key_code: KeyCode,
  ) -> Result<Option<bool>, Box<dyn std::error::Error>> {
    match key_code {
      KeyCode::Char('g') => {
        // Handle 'g' prefix commands
        let inner_key = if self.tutorial_demo_mode {
          if let Some(next_key) = self.check_demo_progress() {
            next_key
          } else {
            return Ok(None);
          }
        } else {
          match event::read()? {
            CEvent::Key(k) => k,
            _ => return Ok(None),
          }
        };
        match inner_key.code {
          KeyCode::Char('g') => {
            // 'gg' - go to first line with overscroll
            self.goto_line_with_overscroll(0);
          }
          KeyCode::Char('v') => {
            // 'gv' - restore last visual selection
            self.restore_visual_selection();
          }
          _ => {
            // Unknown 'g' command - do nothing
          }
        }
        Ok(Some(false))
      }
      KeyCode::Char('G') => {
        // 'G' - go to last line or specific line number with overscroll
        if self.number_prefix.is_empty() {
          // No number prefix - go to last line
          let last_line = self.total_lines.saturating_sub(1);
          self.goto_line_with_overscroll(last_line);
        } else {
          // Number prefix - go to specific line
          if let Ok(line_num) = self.number_prefix.parse::<usize>() {
            let target_line = if line_num > 0 { line_num - 1 } else { 0 }; // Convert to 0-based
            let target_line =
              target_line.min(self.total_lines.saturating_sub(1));
            self.goto_line_with_overscroll(target_line);
          }
          self.number_prefix.clear();
        }
        Ok(Some(false))
      }
      KeyCode::Char('0') => {
        // '0' - jump to start of line
        self.cursor_x = 0;
        Ok(Some(false))
      }
      KeyCode::Char('$') => {
        // '$' - jump to end of line
        let current_line_idx = self.offset + self.cursor_y;
        if current_line_idx < self.lines.len() {
          let line_length = self.lines[current_line_idx].len();
          self.cursor_x = if line_length > 0 { line_length - 1 } else { 0 };
        }
        Ok(Some(false))
      }
      KeyCode::Char('^') => {
        // '^' - jump to first non-whitespace character
        let current_line_idx = self.offset + self.cursor_y;
        if current_line_idx < self.lines.len() {
          let line = &self.lines[current_line_idx];
          for (idx, c) in line.char_indices() {
            if !c.is_whitespace() {
              self.cursor_x = idx;
              return Ok(Some(false));
            }
          }
          // If line is all whitespace, go to start
          self.cursor_x = 0;
        }
        Ok(Some(false))
      }
      KeyCode::Char('%') => {
        // Jump to matching bracket/parenthesis
        if let Some((line, col)) = self.find_matching_bracket() {
          self.move_to_position(line, col);
        }
        Ok(Some(false))
      }
      _ => Ok(None),
    }
  }

  // Handle text object navigation ({}()HML)
  pub fn handle_text_object_keys(
    &mut self,
    key_code: KeyCode,
  ) -> Result<Option<bool>, Box<dyn std::error::Error>> {
    match key_code {
      KeyCode::Char('{') => {
        // Previous paragraph
        let (line, col) = self.find_paragraph_boundary(false);
        self.move_to_position(line, col);
        Ok(Some(false))
      }
      KeyCode::Char('}') => {
        // Next paragraph
        let (line, col) = self.find_paragraph_boundary(true);
        self.move_to_position(line, col);
        Ok(Some(false))
      }
      KeyCode::Char('(') => {
        // Previous sentence
        let (line, col) = self.find_sentence_boundary(false);
        self.move_to_position(line, col);
        Ok(Some(false))
      }
      KeyCode::Char(')') => {
        // Next sentence
        let (line, col) = self.find_sentence_boundary(true);
        self.move_to_position(line, col);
        Ok(Some(false))
      }
      KeyCode::Char('H') => {
        // High - top of screen
        let (line, col) = self.get_screen_position('H');
        self.move_to_position(line, col);
        Ok(Some(false))
      }
      KeyCode::Char('M') => {
        // Middle - middle of screen
        let (line, col) = self.get_screen_position('M');
        self.move_to_position(line, col);
        Ok(Some(false))
      }
      KeyCode::Char('L') => {
        // Low - bottom of screen
        let (line, col) = self.get_screen_position('L');
        self.move_to_position(line, col);
        Ok(Some(false))
      }
      _ => Ok(None),
    }
  }

  // Handle marks and bookmarks (m/')
  pub fn handle_mark_keys(
    &mut self,
    key_code: KeyCode,
  ) -> Result<Option<bool>, Box<dyn std::error::Error>> {
    match key_code {
      KeyCode::Char('m') => {
        // Set mark
        let mark_key = if self.tutorial_demo_mode {
          if let Some(next_key) = self.check_demo_progress() {
            next_key
          } else {
            return Ok(None);
          }
        } else {
          match event::read()? {
            CEvent::Key(k) => k,
            _ => return Ok(None),
          }
        };
        if let KeyCode::Char(mark_char) = mark_key.code
          && mark_char.is_ascii_lowercase()
        {
          let (line, col) = self.get_cursor_position();
          self.marks.insert(mark_char, (line, col));
          self.save_bookmarks();
          self.enqueue_bookmark_sync(mark_char, line, col, false);
        }
        Ok(Some(false))
      }
      KeyCode::Char('\'') => {
        // Jump to mark or previous position
        let mark_key = if self.tutorial_demo_mode {
          if let Some(next_key) = self.check_demo_progress() {
            next_key
          } else {
            return Ok(None);
          }
        } else {
          match event::read()? {
            CEvent::Key(k) => k,
            _ => return Ok(None),
          }
        };
        match mark_key.code {
          KeyCode::Char('\'') => {
            // '' - jump to previous position
            if let Some((line, col)) = self.previous_position {
              let current_pos = self.get_cursor_position();
              self.previous_position = Some(current_pos);
              self.move_to_position(line, col);
            }
          }
          KeyCode::Char(mark_char) if mark_char.is_ascii_lowercase() => {
            // '{mark} - jump to mark
            if let Some(&(line, col)) = self.marks.get(&mark_char) {
              let current_pos = self.get_cursor_position();
              self.previous_position = Some(current_pos);
              self.move_to_position(line, col);
              // Track bookmark jump for tutorial
              if self.tutorial_active {
                self.tutorial_bookmark_jumped = true;
              }
            }
          }
          _ => {}
        }
        Ok(Some(false))
      }
      _ => Ok(None),
    }
  }
}
