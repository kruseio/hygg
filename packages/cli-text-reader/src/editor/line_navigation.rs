use super::char_boundary::{
  ByteCol, CursorCol, Pos, char_starts, grapheme_base_chars,
};
use super::core::{Editor, ViewMode};

impl Editor {
  // Helper to find the start of the next non-empty line
  pub fn find_next_line_start(&self, start_line: usize) -> Pos {
    for line_idx in start_line..self.lines.len() {
      let line = &self.lines[line_idx];
      for (col_idx, c) in char_starts(line) {
        if !c.is_whitespace() {
          return Pos::new(line_idx, col_idx);
        }
      }
    }
    // If no non-whitespace found, go to start of the target line
    if start_line < self.lines.len() {
      Pos::new(start_line, ByteCol::ZERO)
    } else {
      Pos::new(self.lines.len().saturating_sub(1), ByteCol::ZERO)
    }
  }

  // Helper to find the end of the previous line (last word)
  pub fn find_prev_line_end(&self, target_line: usize) -> Pos {
    if target_line >= self.lines.len() {
      return Pos::new(0, ByteCol::ZERO);
    }

    let line = &self.lines[target_line];
    // Walk graphemes, return the byte offset of the found one (issue #99).
    let chars = grapheme_base_chars(line);

    // Find the last non-whitespace character
    let mut end_pos = chars.len();
    while end_pos > 0 {
      if !chars[end_pos - 1].is_whitespace() {
        break;
      }
      end_pos -= 1;
    }

    // Now find the start of the last word
    let mut start_pos = end_pos;
    if start_pos > 0 {
      let char_at_end = chars[start_pos - 1];

      if Editor::is_word_char(char_at_end) {
        // Find start of word characters
        while start_pos > 0 {
          let prev_char = chars[start_pos - 1];
          if !Editor::is_word_char(prev_char) {
            break;
          }
          start_pos -= 1;
        }
      } else {
        // Find start of punctuation
        while start_pos > 0 {
          let prev_char = chars[start_pos - 1];
          if prev_char.is_whitespace() || Editor::is_word_char(prev_char) {
            break;
          }
          start_pos -= 1;
        }
      }
    }

    Pos::new(target_line, ByteCol::from_grapheme_index(line, start_pos))
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

    // First non-whitespace character on the line, or its start
    self.cursor_x = self
      .lines
      .get(target_line)
      .map_or(CursorCol::ZERO, |line| CursorCol::first_non_blank(line));
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
  pub fn find_paragraph_boundary(&self, forward: bool) -> Pos {
    let current_line = self.get_cursor_position().line;

    if forward {
      // Find next paragraph (next blank line or end of document)
      for line_idx in (current_line + 1)..self.lines.len() {
        if self.lines[line_idx].trim().is_empty() {
          // Found blank line, move to next non-blank line or stay here
          for next_idx in (line_idx + 1)..self.lines.len() {
            if !self.lines[next_idx].trim().is_empty() {
              return Pos::new(next_idx, ByteCol::ZERO);
            }
          }
          // If no non-blank line found after, go to the blank line
          return Pos::new(line_idx, ByteCol::ZERO);
        }
      }
      // If no blank line found, go to last line
      Pos::new(self.lines.len().saturating_sub(1), ByteCol::ZERO)
    } else {
      // Find previous paragraph (previous blank line or start of document)
      if current_line == 0 {
        return Pos::new(0, ByteCol::ZERO);
      }

      for line_idx in (0..current_line).rev() {
        if self.lines[line_idx].trim().is_empty() {
          // Found blank line, move to previous non-blank line or stay here
          if line_idx > 0 {
            for prev_idx in (0..line_idx).rev() {
              if !self.lines[prev_idx].trim().is_empty() {
                return Pos::new(prev_idx + 1, ByteCol::ZERO); // Start of paragraph after blank line
              }
            }
          }
          return Pos::new(0, ByteCol::ZERO); // Go to very beginning
        }
      }
      // If no blank line found, go to first line
      Pos::new(0, ByteCol::ZERO)
    }
  }

  // Sentence movement (( and ) commands)
  //
  // Columns in and out are byte offsets; the search itself walks graphemes
  // and maps each hit back with `ByteCol::from_grapheme_index` (issue #99).
  pub fn find_sentence_boundary(&self, forward: bool) -> Pos {
    let Pos { line: line_idx, col: col_idx } = self.get_cursor_position();
    let grapheme_col =
      self.lines.get(line_idx).map_or(0, |line| col_idx.grapheme_index(line));

    if forward {
      // Find next sentence (next period, exclamation, or question mark followed
      // by space/newline)
      let mut search_line = line_idx;
      let mut search_col = grapheme_col + 1;

      while search_line < self.lines.len() {
        let line = &self.lines[search_line];
        let chars = grapheme_base_chars(line);

        while search_col < chars.len() {
          let current_char = chars[search_col];
          if matches!(current_char, '.' | '!' | '?') {
            // Check if followed by space or end of line
            if search_col + 1 >= chars.len()
              || chars[search_col + 1].is_whitespace()
            {
              // Found sentence end, find start of next sentence
              let mut next_col = search_col + 1;

              // Skip whitespace on same line
              while next_col < chars.len() {
                if !chars[next_col].is_whitespace() {
                  return Pos::new(
                    search_line,
                    ByteCol::from_grapheme_index(line, next_col),
                  );
                }
                next_col += 1;
              }

              // Check next lines for sentence start
              for next_line in (search_line + 1)..self.lines.len() {
                let next_line_text = &self.lines[next_line];
                for (idx, c) in char_starts(next_line_text) {
                  if !c.is_whitespace() {
                    return Pos::new(next_line, idx);
                  }
                }
              }

              // If no next sentence found, stay at end
              return Pos::new(
                search_line,
                ByteCol::from_grapheme_index(line, search_col),
              );
            }
          }
          search_col += 1;
        }

        search_line += 1;
        search_col = 0;
      }

      // No sentence boundary found, go to end
      Pos::new(
        self.lines.len().saturating_sub(1),
        self.lines.last().map_or(ByteCol::ZERO, |l| ByteCol::last_grapheme(l)),
      )
    } else {
      // Find previous sentence
      let mut search_line = line_idx;
      let mut search_col = grapheme_col.saturating_sub(1);

      loop {
        let line = &self.lines[search_line];
        let chars = grapheme_base_chars(line);

        while search_col > 0 {
          let current_char = chars.get(search_col).copied().unwrap_or(' ');
          if matches!(current_char, '.' | '!' | '?') {
            // Check if followed by space or end of line
            if search_col + 1 >= chars.len()
              || chars[search_col + 1].is_whitespace()
            {
              // Found sentence end, this is our target
              return Pos::new(
                search_line,
                ByteCol::from_grapheme_index(line, search_col + 1),
              );
            }
          }
          search_col = search_col.saturating_sub(1);
        }

        if search_line == 0 {
          break;
        }
        search_line -= 1;
        search_col =
          grapheme_base_chars(&self.lines[search_line]).len().saturating_sub(1);
      }

      // No sentence boundary found, go to beginning
      Pos::new(0, ByteCol::ZERO)
    }
  }

  // Screen position navigation (H, M, L)
}

#[cfg(test)]
mod tests;
