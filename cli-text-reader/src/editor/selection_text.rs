use super::core::Editor;

impl Editor {
  // Extract selected text based on start and end positions
  pub fn get_selected_text(&self) -> String {
    if let (Some(start), Some(end)) =
      (self.editor_state.selection_start, self.editor_state.selection_end)
    {
      let (start_line, start_col) = if start.0 <= end.0 { start } else { end };

      let (end_line, end_col) = if start.0 <= end.0 { end } else { start };

      // Handle visual line mode vs character mode differently
      if self.editor_state.mode == super::core::EditorMode::VisualLine {
        // In line mode, select entire lines
        if start_line < self.lines.len() && end_line < self.lines.len() {
          return (start_line..=end_line)
            .filter(|line| !self.is_ansi_art_line(*line))
            .map(|line| self.lines[line].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        }
      } else {
        // In character mode, select parts of lines
        if start_line == end_line {
          // Single line selection
          if start_line < self.lines.len() {
            if self.is_ansi_art_line(start_line) {
              return String::new();
            }
            let line = &self.lines[start_line];
            let start_col = start_col.min(line.len());
            let end_col = end_col.min(line.len());
            if start_col < end_col {
              return line[start_col..end_col].to_string();
            } else {
              return line[end_col..start_col].to_string();
            }
          }
        } else {
          // Multi-line selection
          let mut result = String::new();

          // First line (partial)
          if start_line < self.lines.len() {
            if !self.is_ansi_art_line(start_line) {
              let line = &self.lines[start_line];
              let start_col = start_col.min(line.len());
              result.push_str(&line[start_col..]);
              result.push('\n');
            }
          }

          // Middle lines (full)
          for i in start_line + 1..end_line {
            if i < self.lines.len() && !self.is_ansi_art_line(i) {
              result.push_str(&self.lines[i]);
              result.push('\n');
            }
          }

          // Last line (partial)
          if end_line < self.lines.len() {
            if !self.is_ansi_art_line(end_line) {
              let line = &self.lines[end_line];
              let end_col = end_col.min(line.len());
              result.push_str(&line[..end_col]);
            }
          }

          return result;
        }
      }
    }

    String::new()
  }
}
