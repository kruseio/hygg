use super::char_boundary::{ByteCol, Pos, grapheme_base_chars, slice};
use super::core::Editor;

impl Editor {
  // Helper to check if character is a word character (alphanumeric +
  // underscore)
  pub fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
  }

  // Helper to find word boundaries for motions
  pub fn find_word_boundary(&self, forward: bool, big_word: bool) -> Pos {
    let Pos { line: line_idx, col: col_idx } = self.get_cursor_position();

    if line_idx >= self.lines.len() {
      return Pos::new(line_idx, col_idx);
    }

    let line = &self.lines[line_idx];
    // The cursor column is a byte offset. Walk grapheme clusters, each
    // classified by its first char, then map the result back to a byte offset,
    // so a combining mark or a ZWJ tail is never a word of its own and the
    // cursor always lands on a grapheme start (issue #99).
    let chars = grapheme_base_chars(line);
    let grapheme_col = col_idx.grapheme_index(line);

    if forward {
      // Moving forward to next word (vim 'w' behavior)
      let mut new_col = grapheme_col;

      if new_col >= chars.len() {
        // At end of line, move to next line
        if line_idx + 1 < self.lines.len() {
          return self.find_next_line_start(line_idx + 1);
        }
        return Pos::new(line_idx, col_idx);
      }

      let current_char = chars[new_col];

      if big_word {
        // For big words (W), only whitespace separates words
        // Skip current non-whitespace sequence
        while new_col < chars.len() {
          let c = chars[new_col];
          if c.is_whitespace() {
            break;
          }
          new_col += 1;
        }
      } else {
        // For regular words (w), punctuation and alphanumeric are separate
        if Self::is_word_char(current_char) {
          // Skip current word characters
          while new_col < chars.len() {
            let c = chars[new_col];
            if !Self::is_word_char(c) {
              break;
            }
            new_col += 1;
          }
        } else if !current_char.is_whitespace() {
          // Skip current punctuation sequence
          while new_col < chars.len() {
            let c = chars[new_col];
            if c.is_whitespace() || Self::is_word_char(c) {
              break;
            }
            new_col += 1;
          }
        }
      }

      // Skip whitespace to find start of next word
      while new_col < chars.len() {
        let c = chars[new_col];
        if !c.is_whitespace() {
          break;
        }
        new_col += 1;
      }

      if new_col >= chars.len() {
        // Reached end of line, move to next line
        if line_idx + 1 < self.lines.len() {
          return self.find_next_line_start(line_idx + 1);
        }
      }

      Pos::new(line_idx, ByteCol::from_grapheme_index(line, new_col))
    } else {
      // Moving backward to previous word (vim 'b' behavior)
      if grapheme_col == 0 {
        // At beginning of line, go to end of previous line
        if line_idx > 0 {
          return self.find_prev_line_end(line_idx - 1);
        }
        return Pos::new(line_idx, col_idx);
      }

      let mut new_col = grapheme_col.saturating_sub(1);

      // Skip whitespace backwards
      while new_col > 0 {
        let c = chars[new_col];
        if !c.is_whitespace() {
          break;
        }
        new_col = new_col.saturating_sub(1);
      }

      // Now find the start of the current word
      let char_at_pos = chars[new_col];

      if big_word {
        // For big words, find start of non-whitespace sequence
        while new_col > 0 {
          let prev_char = chars[new_col - 1];
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
            let prev_char = chars[new_col - 1];
            if !Self::is_word_char(prev_char) {
              break;
            }
            new_col -= 1;
          }
        } else {
          // Find start of punctuation sequence
          while new_col > 0 {
            let prev_char = chars[new_col - 1];
            if prev_char.is_whitespace() || Self::is_word_char(prev_char) {
              break;
            }
            new_col -= 1;
          }
        }
      }

      Pos::new(line_idx, ByteCol::from_grapheme_index(line, new_col))
    }
  }

  // Find end of word/big word
  pub fn find_word_end(&self, big_word: bool) -> Pos {
    let Pos { line: line_idx, col: col_idx } = self.get_cursor_position();

    if line_idx >= self.lines.len() {
      return Pos::new(line_idx, col_idx);
    }

    let line = &self.lines[line_idx];
    // Byte column in, grapheme walk, byte column out (issue #99).
    let chars = grapheme_base_chars(line);
    let mut new_col = col_idx.grapheme_index(line);

    if new_col >= chars.len() {
      // At end of line, move to next line
      if line_idx + 1 < self.lines.len() {
        return self.find_next_line_start(line_idx + 1);
      }
      return Pos::new(line_idx, col_idx);
    }

    let current_char = chars[new_col];

    if big_word {
      // For big words (E), only whitespace separates words
      if current_char.is_whitespace() {
        // Skip whitespace to find start of next word
        while new_col < chars.len() {
          let c = chars[new_col];
          if !c.is_whitespace() {
            break;
          }
          new_col += 1;
        }
      }

      // Find end of current non-whitespace sequence
      while new_col < chars.len() {
        let c = chars[new_col];
        if c.is_whitespace() {
          break;
        }
        new_col += 1;
      }

      // Move back to last non-whitespace character
      if new_col > 0 && new_col <= chars.len() {
        new_col = new_col.saturating_sub(1);
      }
    } else {
      // For regular words (e), punctuation and alphanumeric are separate
      if current_char.is_whitespace() {
        // Skip whitespace to find start of next word
        while new_col < chars.len() {
          let c = chars[new_col];
          if !c.is_whitespace() {
            break;
          }
          new_col += 1;
        }
      }

      if new_col < chars.len() {
        let char_at_pos = chars[new_col];

        if Self::is_word_char(char_at_pos) {
          // Find end of word characters
          while new_col < chars.len() {
            let c = chars[new_col];
            if !Self::is_word_char(c) {
              break;
            }
            new_col += 1;
          }
        } else {
          // Find end of punctuation sequence
          while new_col < chars.len() {
            let c = chars[new_col];
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

    if new_col >= chars.len() {
      // Reached end of line, move to next line
      if line_idx + 1 < self.lines.len() {
        return self.find_next_line_start(line_idx + 1);
      }
      new_col = chars.len().saturating_sub(1);
    }

    Pos::new(line_idx, ByteCol::from_grapheme_index(line, new_col))
  }

  // Get word under cursor for * and # commands
  pub fn get_word_under_cursor(&self) -> Option<String> {
    let Pos { line: line_idx, col: col_idx } = self.get_cursor_position();

    if line_idx >= self.lines.len() {
      return None;
    }

    let line = &self.lines[line_idx];
    if col_idx >= ByteCol::end(line) {
      return None;
    }

    // Walk graphemes from the cursor's grapheme, then slice by the byte
    // offsets of the word's first and one-past-last graphemes, so trailing
    // combining marks stay part of the word.
    let chars = grapheme_base_chars(line);
    let grapheme_col = col_idx.grapheme_index(line);

    let current_char = *chars.get(grapheme_col)?;
    if !Self::is_word_char(current_char) {
      return None;
    }

    // Find start of word
    let mut start = grapheme_col;
    while start > 0 && Self::is_word_char(chars[start - 1]) {
      start -= 1;
    }

    // Find end of word
    let mut end = grapheme_col;
    while end < chars.len() && Self::is_word_char(chars[end]) {
      end += 1;
    }

    let start = ByteCol::from_grapheme_index(line, start);
    let end = ByteCol::from_grapheme_index(line, end);
    Some(slice(line, start, end).to_string())
  }
}

#[cfg(test)]
mod tests;
