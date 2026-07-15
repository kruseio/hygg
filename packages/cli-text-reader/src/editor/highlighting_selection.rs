use crossterm::{
  QueueableCommand, execute,
  style::{Color, ResetColor, SetBackgroundColor, SetForegroundColor},
};
use std::io::{Result as IoResult, Write};

use super::core::{Editor, EditorMode};

impl Editor {
  // Highlight selected text in visual modes
  pub fn highlight_selection(
    &self,
    stdout: &mut impl Write,
    line_index: usize,
    line: &str,
    center_offset_string: &str,
  ) -> IoResult<bool> {
    if (self.editor_state.mode == EditorMode::VisualChar
      || self.editor_state.mode == EditorMode::VisualLine
      || self.editor_state.visual_selection_active)
      && let (Some(start), Some(end)) =
        (self.editor_state.selection_start, self.editor_state.selection_end)
    {
      let current_line_idx = self.offset + line_index;

      // Determine if this line is within the selection range
      // Check if we're in visual line mode or were in visual line mode
      let is_line_mode = self.editor_state.mode == EditorMode::VisualLine
        || (self.editor_state.visual_selection_active
          && self.editor_state.previous_visual_mode
            == Some(EditorMode::VisualLine));
      let is_in_selection_range = if is_line_mode {
        // In line mode, entire lines are selected
        let (min_line, _) = if start.0 <= end.0 { start } else { end };
        let (max_line, _) = if start.0 > end.0 { start } else { end };
        current_line_idx >= min_line && current_line_idx <= max_line
      } else {
        // In char mode, parts of lines are selected
        if start.0 == end.0 {
          // Single line selection
          current_line_idx == start.0
        } else {
          // Multi-line selection
          let (min_line, _) = if start.0 <= end.0 { start } else { end };
          let (max_line, _) = if start.0 > end.0 { start } else { end };
          current_line_idx >= min_line && current_line_idx <= max_line
        }
      };

      if is_in_selection_range {
        // This line is part of the selection, highlight it
        write!(stdout, "{center_offset_string}")?;

        if self.editor_state.mode == EditorMode::VisualLine {
          // In line mode, highlight the entire line
          execute!(
            stdout,
            SetBackgroundColor(Color::DarkBlue),
            SetForegroundColor(Color::White)
          )?;
          write!(stdout, "{line}")?;
          execute!(stdout, ResetColor)?;
          execute!(
            stdout,
            crossterm::terminal::Clear(
              crossterm::terminal::ClearType::UntilNewLine
            )
          )?;
          return Ok(true);
        } else {
          // In char mode, highlight only the selected portion
          let (start_col, end_col) = if start.0 == end.0 {
            // Selection is within the same line
            if start.1 <= end.1 { (start.1, end.1) } else { (end.1, start.1) }
          } else if current_line_idx == start.0 {
            // First line of multi-line selection
            let (min_line, min_col) =
              if start.0 <= end.0 { start } else { end };
            if current_line_idx == min_line {
              (min_col, line.len())
            } else {
              (0, line.len())
            }
          } else if current_line_idx == end.0 {
            // Last line of multi-line selection
            let (max_line, max_col) = if start.0 > end.0 { start } else { end };
            if current_line_idx == max_line {
              (0, max_col)
            } else {
              (0, line.len())
            }
          } else {
            // Middle line of multi-line selection
            (0, line.len())
          };

          // Ensure column indices are valid
          let start_col = start_col.min(line.len());
          let end_col = end_col.min(line.len());

          // Print parts of the line with appropriate highlighting
          write!(stdout, "{}", &line[..start_col])?;
          execute!(
            stdout,
            SetBackgroundColor(Color::DarkBlue),
            SetForegroundColor(Color::White)
          )?;
          write!(stdout, "{}", &line[start_col..end_col])?;
          execute!(stdout, ResetColor)?;
          write!(stdout, "{}", &line[end_col..])?;
          execute!(
            stdout,
            crossterm::terminal::Clear(
              crossterm::terminal::ClearType::UntilNewLine
            )
          )?;

          return Ok(true);
        }
      }
    }

    Ok(false)
  }

  // Check if a line has visual selection
  pub fn has_selection_on_line(&self, line_index: usize) -> bool {
    if self.editor_state.selection_start.is_none()
      || self.editor_state.selection_end.is_none()
    {
      return false;
    }

    let current_line_idx = self.offset + line_index;
    let start = self.editor_state.selection_start.unwrap();
    let end = self.editor_state.selection_end.unwrap();

    // Check if line is in selection range
    let (min_line, _) = if start.0 <= end.0 { start } else { end };
    let (max_line, _) = if start.0 > end.0 { start } else { end };

    current_line_idx >= min_line && current_line_idx <= max_line
  }

  // Buffered version of highlight_selection
  pub fn highlight_selection_buffered(
    &self,
    buffer: &mut Vec<u8>,
    line_index: usize,
    line: &str,
    center_offset_string: &str,
  ) -> IoResult<bool> {
    if (self.editor_state.mode == EditorMode::VisualChar
      || self.editor_state.mode == EditorMode::VisualLine
      || self.editor_state.visual_selection_active)
      && let (Some(start), Some(end)) =
        (self.editor_state.selection_start, self.editor_state.selection_end)
    {
      let current_line_idx = self.offset + line_index;

      // Determine if this line is within the selection range
      let is_line_mode = self.editor_state.mode == EditorMode::VisualLine
        || (self.editor_state.visual_selection_active
          && self.editor_state.previous_visual_mode
            == Some(EditorMode::VisualLine));
      let is_in_selection_range = if is_line_mode {
        // In line mode, entire lines are selected
        let (min_line, _) = if start.0 <= end.0 { start } else { end };
        let (max_line, _) = if start.0 > end.0 { start } else { end };
        current_line_idx >= min_line && current_line_idx <= max_line
      } else {
        // In char mode, parts of lines are selected
        if start.0 == end.0 {
          // Single line selection
          current_line_idx == start.0
        } else {
          // Multi-line selection
          let (min_line, _) = if start.0 <= end.0 { start } else { end };
          let (max_line, _) = if start.0 > end.0 { start } else { end };
          current_line_idx >= min_line && current_line_idx <= max_line
        }
      };

      if is_in_selection_range {
        // This line is part of the selection, highlight it
        write!(buffer, "{center_offset_string}")?;

        if self.editor_state.mode == EditorMode::VisualLine {
          // In line mode, highlight the entire line
          buffer.queue(SetBackgroundColor(Color::DarkBlue))?;
          buffer.queue(SetForegroundColor(Color::White))?;
          write!(buffer, "{line}")?;
          buffer.queue(ResetColor)?;
          buffer.queue(crossterm::terminal::Clear(
            crossterm::terminal::ClearType::UntilNewLine,
          ))?;
          return Ok(true);
        } else {
          // In char mode, highlight only the selected portion
          let (start_col, end_col) = if start.0 == end.0 {
            // Selection is within the same line
            if start.1 <= end.1 { (start.1, end.1) } else { (end.1, start.1) }
          } else if current_line_idx == start.0 {
            // First line of multi-line selection
            let (min_line, min_col) =
              if start.0 <= end.0 { start } else { end };
            if current_line_idx == min_line {
              (min_col, line.len())
            } else {
              (0, line.len())
            }
          } else if current_line_idx == end.0 {
            // Last line of multi-line selection
            let (max_line, max_col) = if start.0 > end.0 { start } else { end };
            if current_line_idx == max_line {
              (0, max_col)
            } else {
              (0, line.len())
            }
          } else {
            // Middle line of multi-line selection
            (0, line.len())
          };

          // Render the line with selection highlight
          write!(buffer, "{}", &line[..start_col])?;

          buffer.queue(SetBackgroundColor(Color::DarkBlue))?;
          buffer.queue(SetForegroundColor(Color::White))?;
          write!(buffer, "{}", &line[start_col..end_col])?;
          buffer.queue(ResetColor)?;

          write!(buffer, "{}", &line[end_col..])?;
          buffer.queue(crossterm::terminal::Clear(
            crossterm::terminal::ClearType::UntilNewLine,
          ))?;

          return Ok(true);
        }
      }
    }

    Ok(false)
  }
}
