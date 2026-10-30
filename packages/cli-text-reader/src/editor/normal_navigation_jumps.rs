use super::char_boundary::{CursorCol, DisplayCol, Pos};
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
        self.cursor_x = CursorCol::ZERO;
        Ok(Some(false))
      }
      KeyCode::Char('$') => {
        // '$' - jump to end of line
        let current_line_idx = self.offset + self.cursor_y;
        if current_line_idx < self.lines.len() {
          // Start of the last visible character, not `len() - 1`, which
          // lands inside a trailing multi-byte character (issue #99).
          self.cursor_x = CursorCol::last(&self.lines[current_line_idx]);
          // Like vim, `$` makes j/k stay at the end of every line.
          self.remember_display_column(DisplayCol::END);
        }
        Ok(Some(false))
      }
      KeyCode::Char('^') => {
        // '^' - jump to first non-whitespace character
        let current_line_idx = self.offset + self.cursor_y;
        if current_line_idx < self.lines.len() {
          // The start if the line is all whitespace
          self.cursor_x =
            CursorCol::first_non_blank(&self.lines[current_line_idx]);
        }
        Ok(Some(false))
      }
      KeyCode::Char('%') => {
        // Jump to matching bracket/parenthesis
        if let Some(target) = self.find_matching_bracket() {
          self.move_to_position(target);
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
        let target = self.find_paragraph_boundary(false);
        self.move_to_position(target);
        Ok(Some(false))
      }
      KeyCode::Char('}') => {
        // Next paragraph
        let target = self.find_paragraph_boundary(true);
        self.move_to_position(target);
        Ok(Some(false))
      }
      KeyCode::Char('(') => {
        // Previous sentence
        let target = self.find_sentence_boundary(false);
        self.move_to_position(target);
        Ok(Some(false))
      }
      KeyCode::Char(')') => {
        // Next sentence
        let target = self.find_sentence_boundary(true);
        self.move_to_position(target);
        Ok(Some(false))
      }
      KeyCode::Char('H') => {
        // High - top of screen
        let target = self.get_screen_position('H');
        self.move_to_position(target);
        Ok(Some(false))
      }
      KeyCode::Char('M') => {
        // Middle - middle of screen
        let target = self.get_screen_position('M');
        self.move_to_position(target);
        Ok(Some(false))
      }
      KeyCode::Char('L') => {
        // Low - bottom of screen
        let target = self.get_screen_position('L');
        self.move_to_position(target);
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
          let pos = self.get_cursor_position();
          self.marks.insert(mark_char, pos.to_raw());
          self.save_bookmarks();
          let (line, col) = pos.to_raw();
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
            if let Some(target) = self.previous_position {
              let current_pos = self.get_cursor_position();
              self.previous_position = Some(current_pos);
              self.move_to_position(target);
            }
          }
          KeyCode::Char(mark_char) if mark_char.is_ascii_lowercase() => {
            // '{mark} - jump to mark
            if let Some(&raw) = self.marks.get(&mark_char) {
              // Marks come from disk or another device: check the column
              // against this line before jumping.
              let target = Pos::from_raw(&self.lines, raw);
              let current_pos = self.get_cursor_position();
              self.previous_position = Some(current_pos);
              self.move_to_position(target);
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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::editor::char_boundary::CursorCol;

  fn new_editor(lines: &[&str]) -> Editor {
    let mut editor =
      Editor::new(lines.iter().map(|l| l.to_string()).collect(), 80);
    editor.height = 10;
    editor.offset = 0;
    editor.cursor_y = 0;
    editor
  }

  #[test]
  fn dollar_goes_to_start_of_last_multibyte_char() {
    let mut editor = new_editor(&["La canción está aquí", "5€", "日本語", ""]);
    for (row, expected) in [(0, 21), (1, 1), (2, 6), (3, 0)] {
      editor.cursor_y = row;
      editor.cursor_x = CursorCol::lit(0);
      editor.handle_jump_keys(KeyCode::Char('$')).unwrap();
      assert_eq!(editor.cursor_x.raw(), expected, "row {row}");
    }
  }

  #[test]
  fn dollar_goes_to_start_of_last_grapheme() {
    let lines = ["cafe\u{301}", "x👩\u{200d}💻", "🇪🇸🇪🇸", "ที่", "नमस्ते कि"];
    let mut editor = new_editor(&lines);
    for (row, expected) in [(0, 3), (1, 1), (2, 8), (3, 0), (4, 19)] {
      editor.cursor_y = row;
      editor.cursor_x = CursorCol::lit(0);
      editor.handle_jump_keys(KeyCode::Char('$')).unwrap();
      assert_eq!(editor.cursor_x.raw(), expected, "{:?}", lines[row]);
    }
  }
}
