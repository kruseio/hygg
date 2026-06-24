use cli_pdf_to_text::PdfLineKind;
use crossterm::{
  execute,
  style::{Color, ResetColor, SetBackgroundColor, SetForegroundColor},
  terminal::{Clear, ClearType},
};
use std::io::{self, Result as IoResult, Write};

use super::super::core::Editor;

impl Editor {
  // Draw a single pane
  #[allow(clippy::too_many_arguments)]
  pub(crate) fn draw_pane(
    &self,
    stdout: &mut io::Stdout,
    buffer_idx: usize,
    start_row: usize,
    height: usize,
    _term_width: u16,
    center_offset_string: &str,
    _is_active: bool,
  ) -> IoResult<()> {
    self.debug_log(&format!(
      "Drawing pane - buffer: {buffer_idx}, start_row: {start_row}, height: {height}, active: {_is_active}"
    ));

    if let Some(buffer) = self.buffers.get(buffer_idx) {
      // Use current editor state for active buffer, stored state for inactive
      // buffer
      let offset = if buffer_idx == self.active_buffer {
        self.offset
      } else {
        buffer.offset
      };
      let cursor_y = if buffer_idx == self.active_buffer {
        self.cursor_y
      } else {
        buffer.cursor_y
      };
      self.debug_log(&format!(
        "  Buffer {buffer_idx}: offset={offset}, cursor_y={cursor_y}, lines={}",
        buffer.lines.len()
      ));

      for i in 0..height {
        let display_row = start_row + i;
        execute!(stdout, crossterm::cursor::MoveTo(0, display_row as u16))?;

        let line_idx = offset + i;
        if line_idx < buffer.lines.len() {
          let line = buffer.lines[line_idx].clone();

          // Disable cursor line highlighting in split view
          let is_current_line = false;

          // Render the line content
          self.render_pane_line(
            stdout,
            &line,
            buffer_idx,
            i, // Pass viewport line index
            center_offset_string,
            is_current_line,
            offset,             // Pass the offset
            &buffer.lines,      // Pass buffer lines
            &buffer.line_kinds, // Pass buffer line kinds
          )?;

          // Clear to end of line to prevent bleeding
          execute!(stdout, Clear(ClearType::UntilNewLine))?;
        } else {
          // Empty line
          execute!(stdout, Clear(ClearType::CurrentLine))?;
        }
      }
    } else {
      self.debug_log(&format!("Warning: Buffer {buffer_idx} not found"));
      // Clear the entire pane if buffer not found
      for i in 0..height {
        let display_row = start_row + i;
        execute!(stdout, crossterm::cursor::MoveTo(0, display_row as u16))?;
        execute!(stdout, Clear(ClearType::CurrentLine))?;
      }
    }

    Ok(())
  }

  // Render a single line in a pane
  #[allow(clippy::too_many_arguments)]
  pub(crate) fn render_pane_line(
    &self,
    stdout: &mut io::Stdout,
    line: &str,
    buffer_idx: usize,
    viewport_line_idx: usize, // Line index within the pane's viewport
    center_offset_string: &str,
    is_current_line: bool,
    offset: usize, // Offset to use for highlight calculations
    buffer_lines: &[String], // Lines from the buffer being rendered
    buffer_line_kinds: &[PdfLineKind],
  ) -> IoResult<()> {
    // Apply centering if needed
    if let Some(_pane_buffer) = self.buffers.get(buffer_idx) {
      let actual_line_idx = if buffer_idx == self.active_buffer {
        self.offset + viewport_line_idx
      } else if let Some(pane_buffer) = self.buffers.get(buffer_idx) {
        pane_buffer.offset + viewport_line_idx
      } else {
        0
      };
      if self.is_buffer_ansi_art_line(buffer_idx, actual_line_idx) {
        write!(stdout, "{center_offset_string}{line}")?;
        execute!(stdout, ResetColor, Clear(ClearType::UntilNewLine))?;
        return Ok(());
      }

      // Check if this line has visual selection
      let has_selection =
        self.has_pane_selection_on_line(buffer_idx, viewport_line_idx);

      // Check if this line has persistent highlights (only for main buffer)
      let has_persistent = if buffer_idx == 0 {
        self.has_persistent_highlights_on_line_with_offset_lines_and_kinds(
          viewport_line_idx,
          offset,
          buffer_lines,
          buffer_line_kinds,
        )
      } else {
        false
      };

      // If we have multiple types of highlights, use combined rendering
      if (has_selection || has_persistent) && !is_current_line {
        if has_selection && has_persistent {
          // Handle combined highlights
          if self.render_pane_combined_highlights(
            stdout,
            buffer_idx,
            viewport_line_idx,
            line,
            center_offset_string,
            offset,
            buffer_lines,
            buffer_line_kinds,
          )? {
            return Ok(());
          }
        } else if has_selection {
          // Selection only
          if self.render_pane_selection(
            stdout,
            buffer_idx,
            viewport_line_idx,
            line,
            center_offset_string,
          )? {
            return Ok(());
          }
        } else if has_persistent {
          // Persistent highlights only
          if self.render_pane_persistent_highlights(
            stdout,
            buffer_idx,
            viewport_line_idx,
            line,
            center_offset_string,
            offset,
            buffer_lines,
            buffer_line_kinds,
          )? {
            return Ok(());
          }
        }
      }

      // Always apply centering offset for consistency with main display
      let line_to_render = format!("{center_offset_string}{line}");

      // Get the buffer's own search match
      let match_to_highlight = if buffer_idx == self.active_buffer {
        // For the active buffer, use current editor state
        if self.editor_state.search_preview_active {
          self.editor_state.search_preview_match
        } else {
          self.editor_state.current_match
        }
      } else if let Some(pane_buffer) = self.buffers.get(buffer_idx) {
        // For inactive buffer, use stored state
        pane_buffer.current_match
      } else {
        None
      };

      // Check if this line has the match
      if let Some((match_line_idx, start, end)) = match_to_highlight {
        if match_line_idx == actual_line_idx && !is_current_line {
          // Render with match highlighting
          write!(stdout, "{center_offset_string}")?;
          write!(stdout, "{}", &line[..start.min(line.len())])?;
          execute!(
            stdout,
            SetBackgroundColor(Color::Yellow),
            SetForegroundColor(Color::Black)
          )?;
          let end_bounded = end.min(line.len());
          write!(stdout, "{}", &line[start.min(line.len())..end_bounded])?;
          execute!(stdout, ResetColor)?;
          write!(stdout, "{}", &line[end_bounded..])?;
        } else {
          write!(stdout, "{line_to_render}")?;
        }
      } else {
        write!(stdout, "{line_to_render}")?;
      }
    }

    Ok(())
  }

  // Render line with search term highlighted
  #[allow(dead_code)]
  pub(crate) fn render_line_with_search_highlight(
    &self,
    stdout: &mut io::Stdout,
    line: &str,
    search_term: &str,
  ) -> IoResult<()> {
    let mut last_end = 0;
    for (start, part) in line.match_indices(search_term) {
      // Write text before match
      write!(stdout, "{}", &line[last_end..start])?;
      // Write match with highlight
      execute!(
        stdout,
        SetBackgroundColor(Color::Yellow),
        SetForegroundColor(Color::Black)
      )?;
      write!(stdout, "{part}")?;
      execute!(stdout, ResetColor)?;
      last_end = start + part.len();
    }
    // Write remaining text
    write!(stdout, "{}", &line[last_end..])?;
    Ok(())
  }
}
