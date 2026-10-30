use super::char_boundary::{ByteCol, Pos, grapheme_base_chars};
use super::core::Editor;

impl Editor {
  // Select inner paragraph
  pub fn select_inner_paragraph(&mut self) {
    let line_idx = self.get_cursor_position().line;

    // Find paragraph boundaries (empty lines)
    let mut start_line = line_idx;
    let mut end_line = line_idx;

    // Search backward for paragraph start
    while start_line > 0 {
      if self.lines[start_line - 1].trim().is_empty() {
        break;
      }
      start_line -= 1;
    }

    // Search forward for paragraph end
    while end_line < self.lines.len() - 1 {
      if self.lines[end_line + 1].trim().is_empty() {
        break;
      }
      end_line += 1;
    }

    // Set selection
    self.editor_state.selection_start =
      Some(Pos::new(start_line, ByteCol::ZERO));
    self.editor_state.selection_end =
      Some(Pos::new(end_line, ByteCol::end(&self.lines[end_line])));

    // Update buffer selection
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.selection_start = self.editor_state.selection_start;
      buffer.selection_end = self.editor_state.selection_end;
    }
  }

  // Select around paragraph (includes surrounding empty lines)
  pub fn select_around_paragraph(&mut self) {
    let line_idx = self.get_cursor_position().line;

    // Find paragraph boundaries (empty lines)
    let mut start_line = line_idx;
    let mut end_line = line_idx;

    // Search backward for paragraph start
    while start_line > 0 {
      if self.lines[start_line - 1].trim().is_empty() {
        break;
      }
      start_line -= 1;
    }

    // Include empty lines before paragraph
    while start_line > 0 && self.lines[start_line - 1].trim().is_empty() {
      start_line -= 1;
    }

    // Search forward for paragraph end
    while end_line < self.lines.len() - 1 {
      if self.lines[end_line + 1].trim().is_empty() {
        break;
      }
      end_line += 1;
    }

    // Include empty lines after paragraph
    while end_line < self.lines.len() - 1
      && self.lines[end_line + 1].trim().is_empty()
    {
      end_line += 1;
    }

    // Set selection
    self.editor_state.selection_start =
      Some(Pos::new(start_line, ByteCol::ZERO));
    let end_col =
      self.lines.get(end_line).map_or(ByteCol::ZERO, |line| ByteCol::end(line));
    self.editor_state.selection_end = Some(Pos::new(end_line, end_col));

    // Update buffer selection
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.selection_start = self.editor_state.selection_start;
      buffer.selection_end = self.editor_state.selection_end;
    }
  }

  // Select inner sentence
  //
  // The cursor and the stored selection columns are byte offsets. The search
  // walks grapheme clusters (classified by their first char) and maps the
  // ends back with `ByteCol::from_grapheme_index`, so the selection never
  // splits a character, or a visible character, on accented or emoji text
  // (issue #99).
  pub fn select_inner_sentence(&mut self) {
    if self.lines.is_empty() {
      return;
    }
    let cursor = self.get_cursor_position();
    let line_idx = cursor.line;
    let cursor_col = cursor.col.grapheme_index(&self.lines[line_idx]);

    // Find sentence boundaries (. ! ?), in grapheme columns
    let mut start_pos = (line_idx, cursor_col);
    let mut end_pos = (line_idx, cursor_col);

    // Search backward for sentence start
    let mut found_start = false;
    'outer: for search_line in (0..=line_idx).rev() {
      let chars = grapheme_base_chars(&self.lines[search_line]);
      let search_end = if search_line == line_idx {
        (cursor_col + 1).min(chars.len())
      } else {
        chars.len()
      };

      for search_col in (0..search_end).rev() {
        if matches!(chars[search_col], '.' | '!' | '?') {
          // Found sentence end, start is after this
          if search_col + 1 < chars.len() {
            start_pos = (search_line, search_col + 1);
          } else if search_line + 1 < self.lines.len() {
            start_pos = (search_line + 1, 0);
          }
          // Skip whitespace after punctuation
          let start_chars = grapheme_base_chars(&self.lines[start_pos.0]);
          while start_chars.get(start_pos.1).is_some_and(|c| c.is_whitespace())
          {
            start_pos.1 += 1;
          }
          found_start = true;
          break 'outer;
        }
      }
    }

    if !found_start {
      start_pos = (0, 0);
    }

    // Search forward for sentence end
    let mut found_end = false;
    'outer2: for search_line in line_idx..self.lines.len() {
      let chars = grapheme_base_chars(&self.lines[search_line]);
      let search_start = if search_line == line_idx { cursor_col } else { 0 };

      if let Some(search_col) = chars
        .iter()
        .skip(search_start)
        .position(|c| matches!(c, '.' | '!' | '?'))
      {
        end_pos = (search_line, search_start + search_col + 1);
        found_end = true;
        break 'outer2;
      }
    }

    // Back to byte offsets; the end of the line when nothing ends the
    // sentence.
    let to_bytes = |(line, col): (usize, usize)| {
      Pos::new(line, ByteCol::from_grapheme_index(&self.lines[line], col))
    };
    let start_pos = to_bytes(start_pos);
    let end_pos = if found_end {
      to_bytes(end_pos)
    } else {
      let last_line = self.lines.len() - 1;
      Pos::new(last_line, ByteCol::end(&self.lines[last_line]))
    };

    // Set selection
    self.editor_state.selection_start = Some(start_pos);
    self.editor_state.selection_end = Some(end_pos);

    // Update buffer selection
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.selection_start = self.editor_state.selection_start;
      buffer.selection_end = self.editor_state.selection_end;
    }
  }

  // Select around sentence (includes the punctuation and following space)
  pub fn select_around_sentence(&mut self) {
    // First select inner sentence
    self.select_inner_sentence();

    // Extend to include trailing whitespace, a whole grapheme at a time
    if let Some(end) = self.editor_state.selection_end
      && let Some(line) = self.lines.get(end.line)
    {
      let mut new_end_col = end.col;
      while new_end_col.char_at(line).is_some_and(|c| c.is_whitespace()) {
        new_end_col = new_end_col.next_grapheme(line);
      }

      self.editor_state.selection_end = Some(Pos::new(end.line, new_end_col));

      // Update buffer selection
      if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
        buffer.selection_end = self.editor_state.selection_end;
      }
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

  // Selects with `select` from byte `col` of line `row` and returns the
  // selected text, checking the ends are on character boundaries.
  fn selected(
    lines: &[&str],
    row: usize,
    col: usize,
    select: fn(&mut Editor),
  ) -> String {
    let mut editor = new_editor(lines);
    editor.cursor_y = row;
    editor.cursor_x = CursorCol::lit(col);
    select(&mut editor);
    let (sl, sc) = editor.editor_state.selection_start.unwrap().to_raw();
    let (el, ec) = editor.editor_state.selection_end.unwrap().to_raw();
    assert!(lines[sl].is_char_boundary(sc) && lines[el].is_char_boundary(ec));
    if sl == el {
      lines[sl][sc..ec].to_string()
    } else {
      format!("{}\n{}", &lines[sl][sc..], &lines[el][..ec])
    }
  }

  fn inner(e: &mut Editor) {
    e.select_inner_sentence();
  }
  fn around(e: &mut Editor) {
    e.select_around_sentence();
  }

  #[test]
  fn sentence_objects_on_accented_text_use_byte_offsets() {
    // Char-index columns used to start the selection two bytes early here.
    let line = "La canción está. Sí, aquí! Fin";
    let si = line.find("Sí").unwrap();
    assert_eq!(selected(&[line], 0, si, inner), "Sí, aquí!");
    assert_eq!(selected(&[line], 0, si + 1, around), "Sí, aquí! ");
    assert_eq!(selected(&[line], 0, 0, inner), "La canción está.");
    // No terminator: to the end of the line.
    let fin = line.find("Fin").unwrap();
    assert_eq!(selected(&[line], 0, fin, inner), "Fin");
  }

  #[test]
  fn sentence_objects_keep_clusters_whole() {
    let line = "Uno. Dos e\u{301} 👩\u{200d}💻 中文! สวัสดี ที่นี่";
    let dos = "Dos e\u{301} 👩\u{200d}💻 中文!";
    for needle in ["D", "\u{301}", "💻", "文"] {
      let col = line.find(needle).unwrap();
      assert_eq!(selected(&[line], 0, col, inner), dos, "from {needle:?}");
    }
    assert_eq!(
      selected(&[line], 0, line.find('e').unwrap(), around),
      format!("{dos} ")
    );
    // From inside the Thai cluster, to the end of the line.
    let col = line.find("ที่").unwrap() + 3;
    assert_eq!(selected(&[line], 0, col, inner), "สวัสดี ที่นี่");
  }

  #[test]
  fn sentence_objects_across_lines() {
    let lines = ["Hola. Qué", "tal 🇪🇸? Bien"];
    let col = lines[1].find('🇪').unwrap();
    assert_eq!(selected(&lines, 1, col, inner), "Qué\ntal 🇪🇸?");
    // A sentence ending at the end of a line starts on the next one.
    let lines = ["Ya está.", "  ÉL dijo"];
    assert_eq!(selected(&lines, 1, 5, inner), "ÉL dijo");
  }
}
