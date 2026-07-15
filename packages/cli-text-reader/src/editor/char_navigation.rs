use super::core::Editor;

impl Editor {
  // Find the next delimiter in the current line
  pub fn find_text_object(&self, delimiter: char) -> Option<(usize, usize)> {
    let (line_idx, col_idx) = self.get_cursor_position();

    if line_idx >= self.lines.len() {
      return None;
    }

    let line = &self.lines[line_idx];

    // Define opening and closing pairs
    let (opening, closing) = match delimiter {
      '\"' => ('\"', '\"'),
      '\'' => ('\'', '\''),
      '(' | ')' => ('(', ')'),
      '{' | '}' => ('{', '}'),
      '[' | ']' => ('[', ']'),
      _ => return None,
    };

    // Search left for opening delimiter
    let mut start = col_idx;
    let mut depth = 0;

    while start > 0 {
      start -= 1;
      if let Some(c) = line.chars().nth(start) {
        if c == closing {
          depth += 1;
        } else if c == opening {
          if depth == 0 {
            // Found the opening delimiter
            break;
          }
          depth -= 1;
        }
      }
    }

    // Search right for closing delimiter
    let mut end = col_idx;
    depth = 0;

    while end < line.len() {
      if let Some(c) = line.chars().nth(end) {
        if c == opening {
          depth += 1;
        } else if c == closing {
          if depth == 0 {
            // Found the closing delimiter
            break;
          }
          depth -= 1;
        }
      }
      end += 1;
    }

    if end < line.len() && start < end {
      // Return the inner positions (exclude the delimiters)
      return Some((start + 1, end));
    }

    None
  }

  // Find character on current line (f/F/t/T commands)
  pub fn find_char_on_line(
    &self,
    target_char: char,
    forward: bool,
    till: bool,
  ) -> Option<usize> {
    let (line_idx, col_idx) = self.get_cursor_position();

    if line_idx >= self.lines.len() {
      return None;
    }

    let line = &self.lines[line_idx];

    if forward {
      // Search forward from current position + 1
      let start_pos = col_idx + 1;
      if let Some(found_pos) = line[start_pos..].find(target_char) {
        let absolute_pos = start_pos + found_pos;
        if till && absolute_pos > 0 {
          Some(absolute_pos - 1) // Stop before character for 't'
        } else {
          Some(absolute_pos) // Stop on character for 'f'
        }
      } else {
        None
      }
    } else {
      // Search backward from current position - 1
      if col_idx == 0 {
        return None;
      }

      let search_area = &line[..col_idx];
      if let Some(found_pos) = search_area.rfind(target_char) {
        if till {
          Some(found_pos + 1) // Stop after character for 'T'
        } else {
          Some(found_pos) // Stop on character for 'F'
        }
      } else {
        None
      }
    }
  }

  // Find matching bracket/parenthesis (% command)
  pub fn find_matching_bracket(&self) -> Option<(usize, usize)> {
    let (line_idx, col_idx) = self.get_cursor_position();

    if line_idx >= self.lines.len() {
      return None;
    }

    let line = &self.lines[line_idx];
    if col_idx >= line.len() {
      return None;
    }

    let current_char = line.chars().nth(col_idx)?;

    let (opening, closing, forward) = match current_char {
      '(' => ('(', ')', true),
      ')' => ('(', ')', false),
      '{' => ('{', '}', true),
      '}' => ('{', '}', false),
      '[' => ('[', ']', true),
      ']' => ('[', ']', false),
      _ => return None, // Not on a bracket
    };

    if forward {
      // Search forward for closing bracket
      let mut depth = 1;
      let mut search_col = col_idx + 1;

      // Search in current line first
      while search_col < line.len() {
        if let Some(c) = line.chars().nth(search_col) {
          if c == opening {
            depth += 1;
          } else if c == closing {
            depth -= 1;
            if depth == 0 {
              return Some((line_idx, search_col));
            }
          }
        }
        search_col += 1;
      }

      // Search in subsequent lines
      for search_line_idx in (line_idx + 1)..self.lines.len() {
        let search_line = &self.lines[search_line_idx];
        for (char_idx, c) in search_line.char_indices() {
          if c == opening {
            depth += 1;
          } else if c == closing {
            depth -= 1;
            if depth == 0 {
              return Some((search_line_idx, char_idx));
            }
          }
        }
      }
    } else {
      // Search backward for opening bracket
      let mut depth = 1;
      let mut search_col = col_idx;

      // Search in current line first (backwards)
      while search_col > 0 {
        search_col -= 1;
        if let Some(c) = line.chars().nth(search_col) {
          if c == closing {
            depth += 1;
          } else if c == opening {
            depth -= 1;
            if depth == 0 {
              return Some((line_idx, search_col));
            }
          }
        }
      }

      // Search in previous lines
      for search_line_idx in (0..line_idx).rev() {
        let search_line = &self.lines[search_line_idx];
        for (char_idx, c) in search_line.char_indices().rev() {
          if c == closing {
            depth += 1;
          } else if c == opening {
            depth -= 1;
            if depth == 0 {
              return Some((search_line_idx, char_idx));
            }
          }
        }
      }
    }

    None
  }
}
