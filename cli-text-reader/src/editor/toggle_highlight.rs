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
        (Some(start), Some(end)) => {
          // Ensure start comes before end
          if start.0 < end.0 || (start.0 == end.0 && start.1 <= end.1) {
            (start, end)
          } else {
            (end, start)
          }
        }
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
      selection_start.1 = 0;
      if selection_end.0 < self.lines.len() {
        selection_end.1 = self.lines[selection_end.0].len();
      }
    } else if self.editor_state.mode == EditorMode::VisualChar
      || (self.editor_state.visual_selection_active
        && self.editor_state.previous_visual_mode
          == Some(EditorMode::VisualChar))
    {
      // In visual character mode, make selection inclusive (vim-like behavior)
      // We need to include the character at the end position
      if selection_end.0 < self.lines.len() {
        let line_len = self.lines[selection_end.0].len();
        selection_end.1 = (selection_end.1 + 1).min(line_len);
      }
    }

    self.debug_log(&format!(
      "Selection: ({},{}) to ({},{})",
      selection_start.0, selection_start.1, selection_end.0, selection_end.1
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
      self.highlights.add_highlight(start_pos, end_pos);
    } else {
      // Remove all overlapping highlights
      self.debug_log(&format!(
        "Found {} overlapping highlights, removing them",
        overlapping.len()
      ));
      self.highlights.remove_overlapping_highlights(start_pos, end_pos);
    }

    // Save highlights to disk
    self.save_highlights();

    self.debug_log_event("highlight", "toggle_highlight", "completed");
  }

  // Convert visual selection (line, column) to absolute text positions
  fn selection_to_absolute_positions(
    &self,
    start: (usize, usize),
    end: (usize, usize),
  ) -> Option<(usize, usize)> {
    let mut pos = 0;
    let mut start_pos = None;
    let mut end_pos = None;

    for (line_idx, line) in self.lines.iter().enumerate() {
      if line_idx == start.0 {
        start_pos = Some(if self.is_ansi_art_line(line_idx) {
          pos
        } else {
          pos + start.1
        });
      }

      if line_idx == end.0 {
        end_pos =
          Some(if self.is_ansi_art_line(line_idx) { pos } else { pos + end.1 });
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
  pub fn absolute_to_line_column(
    &self,
    abs_pos: usize,
  ) -> Option<(usize, usize)> {
    let mut pos = 0;

    for (line_idx, line) in self.lines.iter().enumerate() {
      if self.is_ansi_art_line(line_idx) {
        continue;
      }
      let line_end = pos + line.len();

      if abs_pos <= line_end {
        return Some((line_idx, abs_pos - pos));
      }

      // Add line length + 1 for newline
      pos = line_end + 1;
    }

    None
  }
}
