use super::core::Editor;

impl Editor {
  // Helper to check if character is a word character (alphanumeric +
  // underscore)
  pub fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
  }

  // Helper to find word boundaries for motions
  pub fn find_word_boundary(
    &self,
    forward: bool,
    big_word: bool,
  ) -> (usize, usize) {
    let (line_idx, col_idx) = self.get_cursor_position();

    if line_idx >= self.lines.len() {
      return (line_idx, col_idx);
    }

    let line = &self.lines[line_idx];

    if forward {
      // Moving forward to next word (vim 'w' behavior)
      let mut new_col = col_idx;

      if new_col >= line.len() {
        // At end of line, move to next line
        if line_idx + 1 < self.lines.len() {
          return self.find_next_line_start(line_idx + 1);
        }
        return (line_idx, col_idx);
      }

      let current_char = line.chars().nth(new_col).unwrap_or(' ');

      if big_word {
        // For big words (W), only whitespace separates words
        // Skip current non-whitespace sequence
        while new_col < line.len() {
          let c = line.chars().nth(new_col).unwrap_or(' ');
          if c.is_whitespace() {
            break;
          }
          new_col += 1;
        }
      } else {
        // For regular words (w), punctuation and alphanumeric are separate
        if Self::is_word_char(current_char) {
          // Skip current word characters
          while new_col < line.len() {
            let c = line.chars().nth(new_col).unwrap_or(' ');
            if !Self::is_word_char(c) {
              break;
            }
            new_col += 1;
          }
        } else if !current_char.is_whitespace() {
          // Skip current punctuation sequence
          while new_col < line.len() {
            let c = line.chars().nth(new_col).unwrap_or(' ');
            if c.is_whitespace() || Self::is_word_char(c) {
              break;
            }
            new_col += 1;
          }
        }
      }

      // Skip whitespace to find start of next word
      while new_col < line.len() {
        let c = line.chars().nth(new_col).unwrap_or(' ');
        if !c.is_whitespace() {
          break;
        }
        new_col += 1;
      }

      if new_col >= line.len() {
        // Reached end of line, move to next line
        if line_idx + 1 < self.lines.len() {
          return self.find_next_line_start(line_idx + 1);
        }
      }

      (line_idx, new_col)
    } else {
      // Moving backward to previous word (vim 'b' behavior)
      if col_idx == 0 {
        // At beginning of line, go to end of previous line
        if line_idx > 0 {
          return self.find_prev_line_end(line_idx - 1);
        }
        return (line_idx, col_idx);
      }

      let mut new_col = col_idx.saturating_sub(1);

      // Skip whitespace backwards
      while new_col > 0 {
        let c = line.chars().nth(new_col).unwrap_or(' ');
        if !c.is_whitespace() {
          break;
        }
        new_col = new_col.saturating_sub(1);
      }

      // Now find the start of the current word
      let char_at_pos = line.chars().nth(new_col).unwrap_or(' ');

      if big_word {
        // For big words, find start of non-whitespace sequence
        while new_col > 0 {
          let prev_char = line.chars().nth(new_col - 1).unwrap_or(' ');
          if prev_char.is_whitespace() {
            break;
          }
          new_col -= 1;
        }
      } else {
        // For regular words, separate by character type
        if Self::is_word_char(char_at_pos) {
          // Find start of word character sequence
          while new_col > 0 {
            let prev_char = line.chars().nth(new_col - 1).unwrap_or(' ');
            if !Self::is_word_char(prev_char) {
              break;
            }
            new_col -= 1;
          }
        } else {
          // Find start of punctuation sequence
          while new_col > 0 {
            let prev_char = line.chars().nth(new_col - 1).unwrap_or(' ');
            if prev_char.is_whitespace() || Self::is_word_char(prev_char) {
              break;
            }
            new_col -= 1;
          }
        }
      }

      (line_idx, new_col)
    }
  }

  // Find end of word/big word
  pub fn find_word_end(&self, big_word: bool) -> (usize, usize) {
    let (line_idx, col_idx) = self.get_cursor_position();

    if line_idx >= self.lines.len() {
      return (line_idx, col_idx);
    }

    let line = &self.lines[line_idx];
    let mut new_col = col_idx;

    if new_col >= line.len() {
      // At end of line, move to next line
      if line_idx + 1 < self.lines.len() {
        return self.find_next_line_start(line_idx + 1);
      }
      return (line_idx, col_idx);
    }

    let current_char = line.chars().nth(new_col).unwrap_or(' ');

    if big_word {
      // For big words (E), only whitespace separates words
      if current_char.is_whitespace() {
        // Skip whitespace to find start of next word
        while new_col < line.len() {
          let c = line.chars().nth(new_col).unwrap_or(' ');
          if !c.is_whitespace() {
            break;
          }
          new_col += 1;
        }
      }

      // Find end of current non-whitespace sequence
      while new_col < line.len() {
        let c = line.chars().nth(new_col).unwrap_or(' ');
        if c.is_whitespace() {
          break;
        }
        new_col += 1;
      }

      // Move back to last non-whitespace character
      if new_col > 0 && new_col <= line.len() {
        new_col = new_col.saturating_sub(1);
      }
    } else {
      // For regular words (e), punctuation and alphanumeric are separate
      if current_char.is_whitespace() {
        // Skip whitespace to find start of next word
        while new_col < line.len() {
          let c = line.chars().nth(new_col).unwrap_or(' ');
          if !c.is_whitespace() {
            break;
          }
          new_col += 1;
        }
      }

      if new_col < line.len() {
        let char_at_pos = line.chars().nth(new_col).unwrap_or(' ');

        if Self::is_word_char(char_at_pos) {
          // Find end of word characters
          while new_col < line.len() {
            let c = line.chars().nth(new_col).unwrap_or(' ');
            if !Self::is_word_char(c) {
              break;
            }
            new_col += 1;
          }
        } else {
          // Find end of punctuation sequence
          while new_col < line.len() {
            let c = line.chars().nth(new_col).unwrap_or(' ');
            if c.is_whitespace() || Self::is_word_char(c) {
              break;
            }
            new_col += 1;
          }
        }

        // Move back to last character of the word
        if new_col > 0 {
          new_col = new_col.saturating_sub(1);
        }
      }
    }

    if new_col >= line.len() {
      // Reached end of line, move to next line
      if line_idx + 1 < self.lines.len() {
        return self.find_next_line_start(line_idx + 1);
      }
      new_col = line.len().saturating_sub(1);
    }

    (line_idx, new_col)
  }

  // Get word under cursor for * and # commands
  pub fn get_word_under_cursor(&self) -> Option<String> {
    let (line_idx, col_idx) = self.get_cursor_position();

    if line_idx >= self.lines.len() {
      return None;
    }

    let line = &self.lines[line_idx];
    if col_idx >= line.len() {
      return None;
    }

    let current_char = line.chars().nth(col_idx)?;
    if !Self::is_word_char(current_char) {
      return None;
    }

    // Find start of word
    let mut start = col_idx;
    while start > 0 {
      if let Some(c) = line.chars().nth(start - 1) {
        if !Self::is_word_char(c) {
          break;
        }
        start -= 1;
      } else {
        break;
      }
    }

    // Find end of word
    let mut end = col_idx;
    while end < line.len() {
      if let Some(c) = line.chars().nth(end) {
        if !Self::is_word_char(c) {
          break;
        }
        end += 1;
      } else {
        break;
      }
    }

    Some(line[start..end].to_string())
  }
}
