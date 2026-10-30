use super::char_boundary::{ByteCol, Pos};
use super::core::{Editor, EditorMode};

impl Editor {
  pub fn toggle_highlight(&mut self) {
    self.debug_log_event("highlight", "toggle_highlight", "starting");

    // Get the active buffer
    let Some(buffer) = self.buffers.get(self.active_buffer) else {
      self.debug_log_error("No active buffer found");
      return;
    };

    // Check if we have a selection
    let (mut selection_start, mut selection_end) =
      match (buffer.selection_start, buffer.selection_end) {
        // Ensure start comes before end
        (Some(start), Some(end)) => (start.min(end), start.max(end)),
        _ => {
          self.debug_log("No visual selection active");
          return;
        }
      };

    // Handle different visual modes
    if self.editor_state.mode == EditorMode::VisualLine
      || (self.editor_state.visual_selection_active
        && self.editor_state.previous_visual_mode
          == Some(EditorMode::VisualLine))
    {
      // In visual line mode, select entire lines
      selection_start.col = ByteCol::ZERO;
      if let Some(line) = self.lines.get(selection_end.line) {
        selection_end.col = ByteCol::end(line);
      }
    } else if self.editor_state.mode == EditorMode::VisualChar
      || (self.editor_state.visual_selection_active
        && self.editor_state.previous_visual_mode
          == Some(EditorMode::VisualChar))
    {
      // In visual character mode, make selection inclusive (vim-like
      // behavior): extend the end past the whole visible character under it,
      // which may be several bytes and several chars wide.
      if let Some(line) = self.lines.get(selection_end.line) {
        selection_end.col = selection_end.col.next_grapheme(line);
      }
    }

    self.debug_log(&format!(
      "Selection: ({},{}) to ({},{})",
      selection_start.line,
      selection_start.col,
      selection_end.line,
      selection_end.col
    ));

    // Convert line/column positions to absolute text positions
    let (start_pos, end_pos) = match self
      .selection_to_absolute_positions(selection_start, selection_end)
    {
      Some((start, end)) => (start, end),
      None => {
        self
          .debug_log_error("Failed to convert selection to absolute positions");
        return;
      }
    };

    self.debug_log(&format!("Absolute positions: {start_pos} to {end_pos}"));

    // Check for overlapping highlights
    let overlapping =
      self.highlights.find_overlapping_highlights(start_pos, end_pos);

    if overlapping.is_empty() {
      // No overlapping highlights, add new one
      self.debug_log("No overlapping highlights found, adding new highlight");
      if self.highlights.add_highlight(start_pos, end_pos) {
        self.enqueue_highlight_sync(start_pos, end_pos, false);
      }
    } else {
      // Remove all overlapping highlights
      self.debug_log(&format!(
        "Found {} overlapping highlights, removing them",
        overlapping.len()
      ));
      let removed =
        self.highlights.remove_overlapping_highlights(start_pos, end_pos);
      // Sync each removal as a tombstone keyed by its exact span.
      for highlight in removed {
        self.enqueue_highlight_sync(highlight.start, highlight.end, true);
      }
    }

    // Save highlights to disk
    self.save_highlights();

    self.debug_log_event("highlight", "toggle_highlight", "completed");
  }

  // Convert visual selection (line, column) to absolute text positions: byte
  // offsets into the lines joined with '\n' (ANSI-art rows excluded). Columns
  // are clamped into their line and floored onto a character boundary so the
  // stored (and server-synced) offsets always slice the text cleanly.
  fn selection_to_absolute_positions(
    &self,
    start: Pos,
    end: Pos,
  ) -> Option<(usize, usize)> {
    let mut pos = 0;
    let mut start_pos = None;
    let mut end_pos = None;

    for (line_idx, line) in self.lines.iter().enumerate() {
      // The absolute offset is this line's start plus the column's bytes.
      let column = |col: ByteCol| {
        if self.is_ansi_art_line(line_idx) {
          pos
        } else {
          pos + col.settle(line).raw()
        }
      };

      if line_idx == start.line {
        start_pos = Some(column(start.col));
      }

      if line_idx == end.line {
        end_pos = Some(column(end.col));
      }

      // If we've found both positions, we can stop
      if start_pos.is_some() && end_pos.is_some() {
        break;
      }

      if !self.is_ansi_art_line(line_idx) {
        // Add line length + 1 for newline.
        pos += line.len() + 1;
      }
    }

    match (start_pos, end_pos) {
      (Some(s), Some(e)) => Some((s.min(e), s.max(e))),
      _ => None,
    }
  }

  // Convert absolute text position to (line, column)
  #[allow(dead_code)]
  pub fn absolute_to_line_column(&self, abs_pos: usize) -> Option<Pos> {
    let mut pos = 0;

    for (line_idx, line) in self.lines.iter().enumerate() {
      if self.is_ansi_art_line(line_idx) {
        continue;
      }
      let line_end = pos + line.len();

      if abs_pos <= line_end {
        // Offsets stored before issue #99 may sit mid-character; floor them.
        return Some(Pos::new(line_idx, ByteCol::floor(line, abs_pos - pos)));
      }

      // Add line length + 1 for newline
      pos = line_end + 1;
    }

    None
  }
}

#[cfg(test)]
mod tests {
  use crate::editor::char_boundary::{ByteCol, Pos};
  use crate::editor::core::{Editor, EditorMode};

  // A selection end as a test fixture: literal, possibly mid-character or past
  // the end, as a stale selection could be.
  fn at((line, col): (usize, usize)) -> Pos {
    Pos::new(line, ByteCol::lit(col))
  }

  fn visual_char_selection(
    lines: &[&str],
    start: (usize, usize),
    end: (usize, usize),
  ) -> Editor {
    let mut editor =
      Editor::new(lines.iter().map(|l| l.to_string()).collect(), 80);
    editor.offset = 0;
    editor.cursor_y = 0;
    editor.set_active_mode(EditorMode::VisualChar);
    editor.editor_state.selection_start = Some(at(start));
    editor.editor_state.selection_end = Some(at(end));
    editor.editor_state.visual_selection_active = true;
    editor.editor_state.previous_visual_mode = Some(EditorMode::VisualChar);
    if let Some(buffer) = editor.buffers.get_mut(0) {
      buffer.selection_start = Some(at(start));
      buffer.selection_end = Some(at(end));
    }
    editor
  }

  fn only_highlight(editor: &Editor) -> (usize, usize) {
    assert_eq!(editor.highlights.highlights.len(), 1);
    let h = &editor.highlights.highlights[0];
    (h.start, h.end)
  }

  #[test]
  fn inclusive_end_covers_the_whole_multibyte_char() {
    // "La canción está aquí": 'ó' is bytes 8..10. Selecting "canció" ends
    // with the cursor on 'ó'; the highlight must end after both of its bytes.
    let line = "La canción está aquí";
    let mut editor = visual_char_selection(&[line], (0, 3), (0, 8));
    editor.toggle_highlight();
    let (start, end) = only_highlight(&editor);
    assert_eq!((start, end), (3, 10));
    assert_eq!(&line[start..end], "canció");
  }

  #[test]
  fn three_byte_char_on_a_later_line_is_fully_included() {
    // Absolute offsets count "First\n" (6 bytes) before the second line.
    // "5€ más": '€' is bytes 1..4 of that line.
    let lines = ["First", "5€ más"];
    let mut editor = visual_char_selection(&lines, (1, 0), (1, 1));
    editor.toggle_highlight();
    let (start, end) = only_highlight(&editor);
    assert_eq!((start, end), (6, 10));
    assert_eq!(&lines.join("\n")[start..end], "5€");
  }

  #[test]
  fn mid_char_and_past_end_columns_land_on_boundaries() {
    // Start inside 'ó' (byte 9) floors to 8; an end past the line clamps to
    // its length.
    let line = "La canción está aquí";
    let mut editor = visual_char_selection(&[line], (0, 9), (0, 99));
    editor.toggle_highlight();
    let (start, end) = only_highlight(&editor);
    assert_eq!((start, end), (8, line.len()));
    assert_eq!(&line[start..end], "ón está aquí");
  }

  #[test]
  fn absolute_to_line_column_floors_mid_char_offsets() {
    let editor = visual_char_selection(&["First", "canción"], (0, 0), (0, 0));
    // "canción" starts at 6; its 'ó' is bytes 5..7 of the line.
    assert_eq!(
      editor.absolute_to_line_column(6 + 5).map(Pos::to_raw),
      Some((1, 5))
    );
    assert_eq!(
      editor.absolute_to_line_column(6 + 6).map(Pos::to_raw),
      Some((1, 5))
    );
    assert_eq!(
      editor.absolute_to_line_column(6 + 8).map(Pos::to_raw),
      Some((1, 8))
    );
    assert_eq!(editor.absolute_to_line_column(6 + 9), None);
  }
}
