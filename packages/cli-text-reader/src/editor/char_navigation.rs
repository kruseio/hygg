use super::char_boundary::{
  ByteCol, CursorCol, Pos, char_starts, find_char_from, rfind_char_before,
};
use super::core::Editor;

impl Editor {
  // Find the next delimiter in the current line
  pub fn find_text_object(
    &self,
    delimiter: char,
  ) -> Option<(ByteCol, ByteCol)> {
    let Pos { line: line_idx, col: col_idx } = self.get_cursor_position();

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

    // Walk characters by index; the cursor column and the returned columns are
    // byte offsets, converted at either end (issue #99).
    let chars: Vec<char> = line.chars().collect();
    let char_col = col_idx.char_index(line);

    // Search left for opening delimiter
    let mut start = char_col;
    let mut depth = 0;

    while start > 0 {
      start -= 1;
      let c = chars[start];
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

    // Search right for closing delimiter
    let mut end = char_col;
    depth = 0;

    while end < chars.len() {
      let c = chars[end];
      if c == opening {
        depth += 1;
      } else if c == closing {
        if depth == 0 {
          // Found the closing delimiter
          break;
        }
        depth -= 1;
      }
      end += 1;
    }

    if end < chars.len() && start < end {
      // Return the inner positions (exclude the delimiters)
      return Some((
        ByteCol::from_char_index(line, start + 1),
        ByteCol::from_char_index(line, end),
      ));
    }

    None
  }

  // Find character on current line (f/F/t/T commands)
  pub fn find_char_on_line(
    &self,
    target_char: char,
    forward: bool,
    till: bool,
  ) -> Option<CursorCol> {
    let Pos { line: line_idx, col: col_idx } = self.get_cursor_position();

    if line_idx >= self.lines.len() {
      return None;
    }

    let line = &self.lines[line_idx];

    // Results are grapheme starts: the cursor never lands inside a visible
    // character, even when the target is a combining mark or the tail of a
    // ZWJ sequence (issue #99).
    if forward {
      // Search forward from the grapheme after the cursor's, which skips the
      // rest of the cursor's own cluster.
      let start = col_idx.next_grapheme(line);
      let found =
        CursorCol::floor(line, find_char_from(line, start, target_char)?);
      if till && found > CursorCol::ZERO {
        // Stop on the grapheme before for 't'
        Some(found.prev(line))
      } else {
        Some(found) // Stop on character for 'f'
      }
    } else {
      // Search backward from the grapheme before the cursor's
      let col_idx = col_idx.floor_grapheme(line);
      if col_idx == ByteCol::ZERO {
        return None;
      }

      let found =
        CursorCol::floor(line, rfind_char_before(line, col_idx, target_char)?);
      if till {
        // Stop on the grapheme after for 'T'
        Some(found.next(line))
      } else {
        Some(found) // Stop on character for 'F'
      }
    }
  }

  // Find matching bracket/parenthesis (% command)
  pub fn find_matching_bracket(&self) -> Option<Pos> {
    let Pos { line: line_idx, col: col_idx } = self.get_cursor_position();

    if line_idx >= self.lines.len() {
      return None;
    }

    let line = &self.lines[line_idx];
    if col_idx >= ByteCol::end(line) {
      return None;
    }

    // Scan with typed char starts so every returned column sits on a
    // character boundary (issue #99).
    let col_idx = col_idx.settle(line);
    let current_char = col_idx.char_at(line)?;

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

      // Search in current line first, after the cursor's character
      for (search_col, c) in char_starts(line).filter(|&(i, _)| i > col_idx) {
        if c == opening {
          depth += 1;
        } else if c == closing {
          depth -= 1;
          if depth == 0 {
            return Some(Pos::new(line_idx, search_col));
          }
        }
      }

      // Search in subsequent lines
      for search_line_idx in (line_idx + 1)..self.lines.len() {
        let search_line = &self.lines[search_line_idx];
        for (char_idx, c) in char_starts(search_line) {
          if c == opening {
            depth += 1;
          } else if c == closing {
            depth -= 1;
            if depth == 0 {
              return Some(Pos::new(search_line_idx, char_idx));
            }
          }
        }
      }
    } else {
      // Search backward for opening bracket
      let mut depth = 1;

      // Search in current line first (backwards), before the cursor
      for (search_col, c) in
        char_starts(line).rev().filter(|&(i, _)| i < col_idx)
      {
        if c == closing {
          depth += 1;
        } else if c == opening {
          depth -= 1;
          if depth == 0 {
            return Some(Pos::new(line_idx, search_col));
          }
        }
      }

      // Search in previous lines
      for search_line_idx in (0..line_idx).rev() {
        let search_line = &self.lines[search_line_idx];
        for (char_idx, c) in char_starts(search_line).rev() {
          if c == closing {
            depth += 1;
          } else if c == opening {
            depth -= 1;
            if depth == 0 {
              return Some(Pos::new(search_line_idx, char_idx));
            }
          }
        }
      }
    }

    None
  }
}

#[cfg(test)]
mod tests;
