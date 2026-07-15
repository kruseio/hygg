use crossterm::{
  QueueableCommand, execute,
  style::{Color, ResetColor, SetBackgroundColor, SetForegroundColor},
};
use std::io::{self, Result as IoResult, Write};

use super::super::core::Editor;

impl Editor {
  // Check if a line in a pane has visual selection
  pub(crate) fn has_pane_selection_on_line(
    &self,
    buffer_idx: usize,
    line_index: usize,
  ) -> bool {
    // Check if selection exists
    let (_has_selection, current_line_idx, start, end) =
      if buffer_idx == self.active_buffer {
        // For active buffer, use current editor state
        let has_sel = self.editor_state.selection_start.is_some()
          && self.editor_state.selection_end.is_some();
        if !has_sel {
          return false;
        }
        (
          has_sel,
          self.offset + line_index,
          self.editor_state.selection_start.unwrap(),
          self.editor_state.selection_end.unwrap(),
        )
      } else if let Some(buffer) = self.buffers.get(buffer_idx) {
        // For inactive buffer, use stored state
        let has_sel =
          buffer.selection_start.is_some() && buffer.selection_end.is_some();
        if !has_sel {
          return false;
        }
        (
          has_sel,
          buffer.offset + line_index,
          buffer.selection_start.unwrap(),
          buffer.selection_end.unwrap(),
        )
      } else {
        return false;
      };

    // Check if line is in selection range
    let (min_line, _) = if start.0 <= end.0 { start } else { end };
    let (max_line, _) = if start.0 > end.0 { start } else { end };

    current_line_idx >= min_line && current_line_idx <= max_line
  }

  // Render visual selection for a pane line
  pub(crate) fn render_pane_selection(
    &self,
    stdout: &mut io::Stdout,
    buffer_idx: usize,
    line_index: usize,
    line: &str,
    center_offset_string: &str,
  ) -> IoResult<bool> {
    if let Some(buffer) = self.buffers.get(buffer_idx) {
      let (start, end, current_line_idx, is_line_mode) =
        if buffer_idx == self.active_buffer {
          // For active buffer, use current editor state
          match (
            self.editor_state.selection_start,
            self.editor_state.selection_end,
          ) {
            (Some(s), Some(e)) => (
              s,
              e,
              self.offset + line_index,
              self.editor_state.mode
                == super::super::core::EditorMode::VisualLine,
            ),
            _ => return Ok(false),
          }
        } else {
          // For inactive buffer, use stored state
          match (buffer.selection_start, buffer.selection_end) {
            (Some(s), Some(e)) => (
              s,
              e,
              buffer.offset + line_index,
              buffer.mode == super::super::core::EditorMode::VisualLine,
            ),
            _ => return Ok(false),
          }
        };

      // Check if this line is in selection
      let (min_line, _) = if start.0 <= end.0 { start } else { end };
      let (max_line, _) = if start.0 > end.0 { start } else { end };

      if current_line_idx >= min_line && current_line_idx <= max_line {
        write!(stdout, "{center_offset_string}")?;

        if is_line_mode {
          // Line mode - highlight entire line
          execute!(
            stdout,
            SetBackgroundColor(Color::DarkBlue),
            SetForegroundColor(Color::White)
          )?;
          write!(stdout, "{line}")?;
          execute!(stdout, ResetColor)?;
          return Ok(true);
        } else {
          // Character mode - highlight selected portion
          let (start_col, end_col) = if start.0 == end.0 {
            // Same line selection
            if start.1 <= end.1 { (start.1, end.1) } else { (end.1, start.1) }
          } else if current_line_idx == min_line {
            // First line of multi-line selection
            if start.0 < end.0 {
              (start.1, line.len())
            } else {
              (end.1, line.len())
            }
          } else if current_line_idx == max_line {
            // Last line of multi-line selection
            if start.0 > end.0 { (0, start.1) } else { (0, end.1) }
          } else {
            // Middle line
            (0, line.len())
          };

          // Ensure indices are valid
          let start_col = start_col.min(line.len());
          let end_col = end_col.min(line.len());

          // Render with selection
          write!(stdout, "{}", &line[..start_col])?;
          execute!(
            stdout,
            SetBackgroundColor(Color::DarkBlue),
            SetForegroundColor(Color::White)
          )?;
          write!(stdout, "{}", &line[start_col..end_col])?;
          execute!(stdout, ResetColor)?;
          write!(stdout, "{}", &line[end_col..])?;

          return Ok(true);
        }
      }
    }
    Ok(false)
  }

  // Buffered version of render_pane_selection
  pub(crate) fn render_pane_selection_buffered(
    &self,
    buf: &mut Vec<u8>,
    buffer_idx: usize,
    line_index: usize,
    line: &str,
    center_offset_string: &str,
  ) -> IoResult<bool> {
    if let Some(buffer) = self.buffers.get(buffer_idx) {
      let (start, end, current_line_idx, is_line_mode) =
        if buffer_idx == self.active_buffer {
          // For active buffer, use current editor state
          match (
            self.editor_state.selection_start,
            self.editor_state.selection_end,
          ) {
            (Some(s), Some(e)) => (
              s,
              e,
              self.offset + line_index,
              self.editor_state.mode
                == super::super::core::EditorMode::VisualLine,
            ),
            _ => return Ok(false),
          }
        } else {
          // For inactive buffer, use stored state
          match (buffer.selection_start, buffer.selection_end) {
            (Some(s), Some(e)) => (
              s,
              e,
              buffer.offset + line_index,
              buffer.mode == super::super::core::EditorMode::VisualLine,
            ),
            _ => return Ok(false),
          }
        };

      // Check if this line is in selection
      let (min_line, _) = if start.0 <= end.0 { start } else { end };
      let (max_line, _) = if start.0 > end.0 { start } else { end };

      if current_line_idx >= min_line && current_line_idx <= max_line {
        write!(buf, "{center_offset_string}")?;

        if is_line_mode {
          // Line mode - highlight entire line
          buf.queue(SetBackgroundColor(Color::DarkBlue))?;
          buf.queue(SetForegroundColor(Color::White))?;
          write!(buf, "{line}")?;
          buf.queue(ResetColor)?;
          return Ok(true);
        } else {
          // Character mode - highlight selected portion
          let (start_col, end_col) = if start.0 == end.0 {
            // Same line selection
            if start.1 <= end.1 { (start.1, end.1) } else { (end.1, start.1) }
          } else if current_line_idx == min_line {
            // First line of multi-line selection
            if start.0 < end.0 {
              (start.1, line.len())
            } else {
              (end.1, line.len())
            }
          } else if current_line_idx == max_line {
            // Last line of multi-line selection
            if start.0 > end.0 { (0, start.1) } else { (0, end.1) }
          } else {
            // Middle line
            (0, line.len())
          };

          // Ensure indices are valid
          let start_col = start_col.min(line.len());
          let end_col = end_col.min(line.len());

          // Render with selection
          write!(buf, "{}", &line[..start_col])?;
          buf.queue(SetBackgroundColor(Color::DarkBlue))?;
          buf.queue(SetForegroundColor(Color::White))?;
          write!(buf, "{}", &line[start_col..end_col])?;
          buf.queue(ResetColor)?;
          write!(buf, "{}", &line[end_col..])?;

          return Ok(true);
        }
      }
    }
    Ok(false)
  }
}
