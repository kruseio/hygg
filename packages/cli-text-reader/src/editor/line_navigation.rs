use super::core::{Editor, ViewMode};

impl Editor {
  // Helper to find the start of the next non-empty line
  pub fn find_next_line_start(&self, start_line: usize) -> (usize, usize) {
    for line_idx in start_line..self.lines.len() {
      let line = &self.lines[line_idx];
      for (col_idx, c) in line.char_indices() {
        if !c.is_whitespace() {
          return (line_idx, col_idx);
        }
      }
    }
    // If no non-whitespace found, go to start of the target line
    if start_line < self.lines.len() {
      (start_line, 0)
    } else {
      (self.lines.len().saturating_sub(1), 0)
    }
  }

  // Helper to find the end of the previous line (last word)
  pub fn find_prev_line_end(&self, target_line: usize) -> (usize, usize) {
    if target_line >= self.lines.len() {
      return (0, 0);
    }

    let line = &self.lines[target_line];

    // Find the last non-whitespace character
    let mut end_pos = line.len();
    while end_pos > 0 {
      let c = line.chars().nth(end_pos - 1).unwrap_or(' ');
      if !c.is_whitespace() {
        break;
      }
      end_pos -= 1;
    }

    // Now find the start of the last word
    let mut start_pos = end_pos;
    if start_pos > 0 {
      let char_at_end = line.chars().nth(start_pos - 1).unwrap_or(' ');

      if Editor::is_word_char(char_at_end) {
        // Find start of word characters
        while start_pos > 0 {
          let prev_char = line.chars().nth(start_pos - 1).unwrap_or(' ');
          if !Editor::is_word_char(prev_char) {
            break;
          }
          start_pos -= 1;
        }
      } else {
        // Find start of punctuation
        while start_pos > 0 {
          let prev_char = line.chars().nth(start_pos - 1).unwrap_or(' ');
          if prev_char.is_whitespace() || Editor::is_word_char(prev_char) {
            break;
          }
          start_pos -= 1;
        }
      }
    }

    (target_line, start_pos)
  }

  // Navigate to a specific line with overscroll capability
  pub fn goto_line_with_overscroll(&mut self, target_line: usize) {
    // Clamp target line to valid range
    let target_line = target_line.min(self.total_lines.saturating_sub(1));

    // Use effective viewport height
    let viewport_height = self.get_effective_viewport_height();
    let center_y = viewport_height / 2;

    // Always try to center the target line (overscroll behavior)
    self.offset = target_line.saturating_sub(center_y);

    // Calculate cursor_y position
    if target_line < center_y {
      // Target line is near the beginning - cursor follows the line
      self.cursor_y = target_line;
      self.offset = 0;
    } else {
      // Center the cursor
      self.cursor_y = center_y;
    }

    // Ensure cursor_y stays within viewport bounds
    let max_cursor_y = viewport_height.saturating_sub(1);
    if self.cursor_y > max_cursor_y {
      self.debug_log(&format!(
        "goto_line_with_overscroll: Adjusting cursor_y {} to max {}",
        self.cursor_y, max_cursor_y
      ));
      self.cursor_y = max_cursor_y;
    }

    // Save state back to buffer if in split view
    if self.view_mode == ViewMode::HorizontalSplit {
      self.save_current_buffer_state();
    }

    self.mark_dirty();

    // Set cursor to beginning of line
    self.cursor_x = 0;

    // Find first non-whitespace character on the line if it exists
    if target_line < self.lines.len() {
      let line = &self.lines[target_line];
      for (idx, c) in line.char_indices() {
        if !c.is_whitespace() {
          self.cursor_x = idx;
          break;
        }
      }
    }
  }

  // Ctrl+U - half page up (vim-like behavior)
  pub fn half_page_up(&mut self) {
    let current_line = self.offset + self.cursor_y;
    let viewport_height = self.get_effective_viewport_height();
    let half_page = (viewport_height / 2).max(1);

    let target_line = current_line.saturating_sub(half_page);
    self.goto_line_with_overscroll(target_line);
  }

  // Ctrl+D - half page down (vim-like behavior)
  pub fn half_page_down(&mut self) {
    let current_line = self.offset + self.cursor_y;
    let viewport_height = self.get_effective_viewport_height();
    let half_page = (viewport_height / 2).max(1);

    let target_line =
      (current_line + half_page).min(self.total_lines.saturating_sub(1));
    self.goto_line_with_overscroll(target_line);
  }

  // Paragraph movement ({ and } commands)
  pub fn find_paragraph_boundary(&self, forward: bool) -> (usize, usize) {
    let (current_line, _) = self.get_cursor_position();

    if forward {
      // Find next paragraph (next blank line or end of document)
      for line_idx in (current_line + 1)..self.lines.len() {
        if self.lines[line_idx].trim().is_empty() {
          // Found blank line, move to next non-blank line or stay here
          for next_idx in (line_idx + 1)..self.lines.len() {
            if !self.lines[next_idx].trim().is_empty() {
              return (next_idx, 0);
            }
          }
          // If no non-blank line found after, go to the blank line
          return (line_idx, 0);
        }
      }
      // If no blank line found, go to last line
      (self.lines.len().saturating_sub(1), 0)
    } else {
      // Find previous paragraph (previous blank line or start of document)
      if current_line == 0 {
        return (0, 0);
      }

      for line_idx in (0..current_line).rev() {
        if self.lines[line_idx].trim().is_empty() {
          // Found blank line, move to previous non-blank line or stay here
          if line_idx > 0 {
            for prev_idx in (0..line_idx).rev() {
              if !self.lines[prev_idx].trim().is_empty() {
                return (prev_idx + 1, 0); // Start of paragraph after blank line
              }
            }
          }
          return (0, 0); // Go to very beginning
        }
      }
      // If no blank line found, go to first line
      (0, 0)
    }
  }

  // Sentence movement (( and ) commands)
  pub fn find_sentence_boundary(&self, forward: bool) -> (usize, usize) {
    let (line_idx, col_idx) = self.get_cursor_position();

    if forward {
      // Find next sentence (next period, exclamation, or question mark followed
      // by space/newline)
      let mut search_line = line_idx;
      let mut search_col = col_idx + 1;

      while search_line < self.lines.len() {
        let line = &self.lines[search_line];

        while search_col < line.len() {
          let current_char = line.chars().nth(search_col).unwrap_or(' ');
          if matches!(current_char, '.' | '!' | '?') {
            // Check if followed by space or end of line
            if search_col + 1 >= line.len()
              || line
                .chars()
                .nth(search_col + 1)
                .is_some_and(|c| c.is_whitespace())
            {
              // Found sentence end, find start of next sentence
              let mut next_col = search_col + 1;

              // Skip whitespace on same line
              while next_col < line.len() {
                if !line.chars().nth(next_col).unwrap_or(' ').is_whitespace() {
                  return (search_line, next_col);
                }
                next_col += 1;
              }

              // Check next lines for sentence start
              for next_line in (search_line + 1)..self.lines.len() {
                let next_line_text = &self.lines[next_line];
                for (idx, c) in next_line_text.char_indices() {
                  if !c.is_whitespace() {
                    return (next_line, idx);
                  }
                }
              }

              // If no next sentence found, stay at end
              return (search_line, search_col);
            }
          }
          search_col += 1;
        }

        search_line += 1;
        search_col = 0;
      }

      // No sentence boundary found, go to end
      (
        self.lines.len().saturating_sub(1),
        self.lines.last().map_or(0, |l| l.len().saturating_sub(1)),
      )
    } else {
      // Find previous sentence
      let mut search_line = line_idx;
      let mut search_col = col_idx.saturating_sub(1);

      loop {
        let line = &self.lines[search_line];

        while search_col > 0 {
          let current_char = line.chars().nth(search_col).unwrap_or(' ');
          if matches!(current_char, '.' | '!' | '?') {
            // Check if followed by space or end of line
            if search_col + 1 >= line.len()
              || line
                .chars()
                .nth(search_col + 1)
                .is_some_and(|c| c.is_whitespace())
            {
              // Found sentence end, this is our target
              return (search_line, search_col + 1);
            }
          }
          search_col = search_col.saturating_sub(1);
        }

        if search_line == 0 {
          break;
        }
        search_line -= 1;
        search_col = self.lines[search_line].len().saturating_sub(1);
      }

      // No sentence boundary found, go to beginning
      (0, 0)
    }
  }

  // Screen position navigation (H, M, L)
}
