use super::core::Editor;

impl Editor {
  // Select inner word
  pub fn select_inner_word(&mut self, big_word: bool) {
    let (line_idx, col_idx) = self.get_cursor_position();

    if line_idx >= self.lines.len() {
      return;
    }

    let line = &self.lines[line_idx];

    if line.is_empty() {
      return;
    }

    // Clamp cursor position to valid range
    let col_idx = col_idx.min(line.len().saturating_sub(1));

    if big_word {
      // Big word (space separated) - non-whitespace sequences
      let is_whitespace =
        |idx: usize| line.chars().nth(idx).is_none_or(|c| c.is_whitespace());

      // If cursor is on whitespace, no word to select
      if is_whitespace(col_idx) {
        return;
      }

      // Find word boundaries
      let mut start = col_idx;
      let mut end = col_idx + 1; // Start with next character

      // Move backward to find start
      while start > 0 && !is_whitespace(start - 1) {
        start -= 1;
      }

      // Move forward to find end
      while end < line.len() && !is_whitespace(end) {
        end += 1;
      }

      // Set selection
      self.editor_state.selection_start = Some((line_idx, start));
      self.editor_state.selection_end = Some((line_idx, end));
      // Also update active buffer's selection
      if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
        buffer.selection_start = Some((line_idx, start));
        buffer.selection_end = Some((line_idx, end));
      }
    } else {
      // Regular word (alphanumeric + underscore) - vim's 'w' behavior
      let is_word_char = |c: char| c.is_alphanumeric() || c == '_';
      let char_at = |idx: usize| line.chars().nth(idx).unwrap_or(' ');

      let current_char = char_at(col_idx);

      // If cursor is on whitespace, no word to select
      if current_char.is_whitespace() {
        return;
      }

      let mut start = col_idx;
      let mut end = col_idx + 1; // Start with next character

      if is_word_char(current_char) {
        // We're on a word character - find word boundaries

        // Move backward to find start of word
        while start > 0 && is_word_char(char_at(start - 1)) {
          start -= 1;
        }

        // Move forward to find end of word
        while end < line.len() && is_word_char(char_at(end)) {
          end += 1;
        }
      } else {
        // We're on punctuation - find punctuation sequence boundaries

        // Move backward to find start of punctuation sequence
        while start > 0 {
          let prev_char = char_at(start - 1);
          if prev_char.is_whitespace() || is_word_char(prev_char) {
            break;
          }
          start -= 1;
        }

        // Move forward to find end of punctuation sequence
        while end < line.len() {
          let next_char = char_at(end);
          if next_char.is_whitespace() || is_word_char(next_char) {
            break;
          }
          end += 1;
        }
      }

      // Set selection
      self.editor_state.selection_start = Some((line_idx, start));
      self.editor_state.selection_end = Some((line_idx, end));
      // Also update active buffer's selection
      if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
        buffer.selection_start = Some((line_idx, start));
        buffer.selection_end = Some((line_idx, end));
      }
    }
  }

  // Select around word (including trailing whitespace)
  pub fn select_around_word(&mut self, big_word: bool) {
    let (line_idx, col_idx) = self.get_cursor_position();

    if line_idx >= self.lines.len() {
      return;
    }

    let line = &self.lines[line_idx];
    if line.is_empty() {
      return;
    }

    // First, find the inner word boundaries
    let chars: Vec<char> = line.chars().collect();
    let mut start = col_idx;

    // If we're on whitespace, find the nearest word
    if col_idx < chars.len() && chars[col_idx].is_whitespace() {
      // Look backward for a word
      while start > 0 && chars[start - 1].is_whitespace() {
        start -= 1;
      }
      if start > 0 {
        start -= 1;
      }
    }

    // Find word start
    if start < chars.len() {
      let char_at_cursor = chars[start];
      if big_word {
        // Big WORD - only whitespace breaks words
        while start > 0 && !chars[start - 1].is_whitespace() {
          start -= 1;
        }
      } else if Editor::is_word_char(char_at_cursor) {
        // Regular word - alphanumeric + underscore
        while start > 0 && Editor::is_word_char(chars[start - 1]) {
          start -= 1;
        }
      } else if !char_at_cursor.is_whitespace() {
        // Punctuation
        while start > 0
          && !chars[start - 1].is_whitespace()
          && !Editor::is_word_char(chars[start - 1])
        {
          start -= 1;
        }
      }
    }

    // Find word end
    let mut end = start;
    if end < chars.len() {
      let char_at_start = chars[end];
      if big_word {
        // Big WORD
        while end < chars.len() && !chars[end].is_whitespace() {
          end += 1;
        }
      } else if Editor::is_word_char(char_at_start) {
        // Regular word
        while end < chars.len() && Editor::is_word_char(chars[end]) {
          end += 1;
        }
      } else if !char_at_start.is_whitespace() {
        // Punctuation
        while end < chars.len()
          && !chars[end].is_whitespace()
          && !Editor::is_word_char(chars[end])
        {
          end += 1;
        }
      }
    }

    // Include trailing whitespace for "around" selection
    while end < chars.len() && chars[end].is_whitespace() {
      end += 1;
    }

    // If we didn't find trailing whitespace, try leading whitespace
    if end < chars.len() || start == 0 {
      while start > 0 && chars[start - 1].is_whitespace() {
        start -= 1;
      }
    }

    // Update selection
    self.editor_state.selection_start = Some((line_idx, start));
    self.editor_state.selection_end = Some((line_idx, end));

    // Update buffer
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.selection_start = Some((line_idx, start));
      buffer.selection_end = Some((line_idx, end));
    }
  }
}
