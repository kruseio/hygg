use cli_pdf_to_text::PdfLineKind;
use crossterm::{
  QueueableCommand, execute,
  style::{Color, ResetColor, SetBackgroundColor, SetForegroundColor},
  terminal::{Clear, ClearType},
};
use std::io::{self, Result as IoResult, Write};

use super::core::{Editor, ViewMode};
use super::highlighting_persistent::HighlightType;

impl Editor {
  // Draw split view with two panes
  pub(super) fn draw_split_view(
    &self,
    stdout: &mut io::Stdout,
    term_width: u16,
    center_offset_string: &str,
  ) -> IoResult<()> {
    self.debug_log("=== draw_split_view ===");
    self.debug_log(&format!(
      "Active pane: {}, Active buffer: {}",
      self.active_pane, self.active_buffer
    ));

    // Calculate pane heights
    let terminal_height = self.height.saturating_sub(1); // Subtract status line
    let top_height = (terminal_height as f32 * self.split_ratio) as usize;
    let bottom_height =
      terminal_height.saturating_sub(top_height).saturating_sub(1); // -1 for separator

    self.debug_log(&format!(
      "Terminal height: {terminal_height}, Top pane: {top_height}, Bottom pane: {bottom_height}"
    ));

    // Determine buffer indices based on tutorial mode
    let (top_buffer_idx, bottom_buffer_idx) = if self.tutorial_active
      && self.buffers.len() > 2
    {
      // In tutorial mode: show tutorial (1) in top, command (2) in bottom
      self
        .debug_log("  Tutorial mode split: tutorial in top, command in bottom");
      (1, 2)
    } else {
      // Normal mode: show main (0) in top, command (1) in bottom
      self.debug_log("  Normal mode split: main in top, command in bottom");
      (0, 1)
    };

    // Draw top pane
    self.draw_pane(
      stdout,
      top_buffer_idx, // buffer index
      0,              // start row
      top_height,
      term_width,
      center_offset_string,
      self.active_pane == 0,
    )?;

    // Draw separator
    execute!(
      stdout,
      crossterm::cursor::MoveTo(0, top_height as u16),
      SetForegroundColor(Color::DarkGrey)
    )?;
    write!(stdout, "{}", "─".repeat(term_width as usize))?;
    execute!(stdout, ResetColor)?;

    // Draw bottom pane
    self.draw_pane(
      stdout,
      bottom_buffer_idx, // buffer index
      top_height + 1,    // start row (after separator)
      bottom_height,
      term_width,
      center_offset_string,
      self.active_pane == 1,
    )?;

    Ok(())
  }

  // Draw a single pane
  #[allow(clippy::too_many_arguments)]
  fn draw_pane(
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
  fn render_pane_line(
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
  fn render_line_with_search_highlight(
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

  // Buffered version of draw_split_view
  pub(super) fn draw_split_view_buffered(
    &self,
    buffer: &mut Vec<u8>,
    term_width: u16,
    center_offset_string: &str,
  ) -> IoResult<()> {
    self.debug_log("=== draw_split_view_buffered ===");
    self.debug_log(&format!(
      "Active pane: {}, Active buffer: {}",
      self.active_pane, self.active_buffer
    ));

    // Calculate pane heights
    let terminal_height = self.height.saturating_sub(1); // Subtract status line
    let top_height = (terminal_height as f32 * self.split_ratio) as usize;
    let bottom_height =
      terminal_height.saturating_sub(top_height).saturating_sub(1); // -1 for separator

    self.debug_log(&format!(
      "Terminal height: {terminal_height}, Top pane: {top_height}, Bottom pane: {bottom_height}"
    ));

    // Determine buffer indices based on tutorial mode
    let (top_buffer_idx, bottom_buffer_idx) = if self.tutorial_active
      && self.buffers.len() > 2
    {
      // In tutorial mode: show tutorial (1) in top, command (2) in bottom
      self
        .debug_log("  Tutorial mode split: tutorial in top, command in bottom");
      (1, 2)
    } else {
      // Normal mode: show main (0) in top, command (1) in bottom
      self.debug_log("  Normal mode split: main in top, command in bottom");
      (0, 1)
    };

    // Draw top pane
    self.draw_pane_buffered(
      buffer,
      top_buffer_idx, // buffer index
      0,              // start row
      top_height,
      term_width,
      center_offset_string,
      self.active_pane == 0,
    )?;

    // Draw separator
    buffer.queue(crossterm::cursor::MoveTo(0, top_height as u16))?;
    buffer.queue(SetForegroundColor(Color::DarkGrey))?;
    write!(buffer, "{}", "─".repeat(term_width as usize))?;
    buffer.queue(ResetColor)?;

    // Draw bottom pane
    self.draw_pane_buffered(
      buffer,
      bottom_buffer_idx, // buffer index
      top_height + 1,    // start row (after separator)
      bottom_height,
      term_width,
      center_offset_string,
      self.active_pane == 1,
    )?;

    Ok(())
  }

  // Buffered version of draw_pane
  #[allow(clippy::too_many_arguments)]
  fn draw_pane_buffered(
    &self,
    buffer: &mut Vec<u8>,
    buffer_idx: usize,
    start_row: usize,
    height: usize,
    _term_width: u16,
    center_offset_string: &str,
    _is_active: bool,
  ) -> IoResult<()> {
    self.debug_log(&format!(
      "Drawing pane buffered - buffer: {buffer_idx}, start_row: {start_row}, height: {height}, active: {_is_active}"
    ));

    if let Some(pane_buffer) = self.buffers.get(buffer_idx) {
      // Use current editor state for active buffer, stored state for inactive
      // buffer
      let offset = if buffer_idx == self.active_buffer {
        self.offset
      } else {
        pane_buffer.offset
      };
      let cursor_y = if buffer_idx == self.active_buffer {
        self.cursor_y
      } else {
        pane_buffer.cursor_y
      };
      self.debug_log(&format!(
        "  Buffer {buffer_idx}: offset={offset}, cursor_y={cursor_y}, lines={}",
        pane_buffer.lines.len()
      ));

      // Draw each line in the pane
      for i in 0..height {
        let display_row = start_row + i;
        buffer.queue(crossterm::cursor::MoveTo(0, display_row as u16))?;

        let line_idx = offset + i;
        if line_idx < pane_buffer.lines.len() {
          let line = &pane_buffer.lines[line_idx];

          // Disable cursor line highlighting in split view
          let is_current_line = false;

          // Render the line
          self.render_pane_line_buffered(
            buffer,
            line,
            buffer_idx,
            i, // Pass viewport line index, not display row
            center_offset_string,
            is_current_line,
            offset,                  // Pass the offset
            &pane_buffer.lines,      // Pass buffer lines
            &pane_buffer.line_kinds, // Pass buffer line kinds
          )?;

          // Clear to end of line
          buffer.queue(Clear(ClearType::UntilNewLine))?;
        } else {
          // Empty line
          buffer.queue(Clear(ClearType::CurrentLine))?;
        }
      }
    } else {
      // Buffer doesn't exist - clear the pane
      for i in 0..height {
        let display_row = start_row + i;
        buffer.queue(crossterm::cursor::MoveTo(0, display_row as u16))?;
        buffer.queue(Clear(ClearType::CurrentLine))?;
      }
    }

    Ok(())
  }

  // Buffered version of render_pane_line
  #[allow(clippy::too_many_arguments)]
  fn render_pane_line_buffered(
    &self,
    buffer: &mut Vec<u8>,
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
        write!(buffer, "{center_offset_string}{line}")?;
        buffer.queue(ResetColor)?;
        buffer.queue(Clear(ClearType::UntilNewLine))?;
        return Ok(());
      }

      // Check if this line has visual selection
      let _start_row = if buffer_idx == 0 {
        0
      } else {
        // For bottom pane, calculate start row based on split ratio
        let terminal_height = self.height.saturating_sub(1);
        let top_height = (terminal_height as f32 * self.split_ratio) as usize;
        top_height + 1
      };
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
          if self.render_pane_combined_highlights_buffered(
            buffer,
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
          if self.render_pane_selection_buffered(
            buffer,
            buffer_idx,
            viewport_line_idx,
            line,
            center_offset_string,
          )? {
            return Ok(());
          }
        } else if has_persistent {
          // Persistent highlights only
          if self.render_pane_persistent_highlights_buffered(
            buffer,
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
          write!(buffer, "{center_offset_string}")?;
          write!(buffer, "{}", &line[..start.min(line.len())])?;
          buffer.queue(SetBackgroundColor(Color::Yellow))?;
          buffer.queue(SetForegroundColor(Color::Black))?;
          let end_bounded = end.min(line.len());
          write!(buffer, "{}", &line[start.min(line.len())..end_bounded])?;
          buffer.queue(ResetColor)?;
          write!(buffer, "{}", &line[end_bounded..])?;
        } else {
          write!(buffer, "{line_to_render}")?;
        }
      } else {
        write!(buffer, "{line_to_render}")?;
      }
    }

    Ok(())
  }

  // Buffered version of render_line_with_search_highlight
  #[allow(dead_code)]
  fn render_line_with_search_highlight_buffered(
    &self,
    buffer: &mut Vec<u8>,
    line: &str,
    search_term: &str,
  ) -> IoResult<()> {
    let mut last_end = 0;
    for (start, part) in line.match_indices(search_term) {
      // Write text before match
      write!(buffer, "{}", &line[last_end..start])?;
      // Write match with highlight
      buffer.queue(SetBackgroundColor(Color::Yellow))?;
      buffer.queue(SetForegroundColor(Color::Black))?;
      write!(buffer, "{part}")?;
      buffer.queue(ResetColor)?;
      last_end = start + part.len();
    }
    // Write remaining text
    write!(buffer, "{}", &line[last_end..])?;
    Ok(())
  }

  // Render combined highlights (selection + persistent) for a pane line
  #[allow(clippy::too_many_arguments)]
  fn render_pane_combined_highlights(
    &self,
    stdout: &mut io::Stdout,
    buffer_idx: usize,
    viewport_line_idx: usize,
    line: &str,
    center_offset_string: &str,
    offset: usize,
    buffer_lines: &[String],
    buffer_line_kinds: &[PdfLineKind],
  ) -> IoResult<bool> {
    // Only main buffer has persistent highlights
    if buffer_idx != 0 {
      return self.render_pane_selection(
        stdout,
        buffer_idx,
        viewport_line_idx,
        line,
        center_offset_string,
      );
    }

    let current_line_idx = offset + viewport_line_idx;

    // Get all highlight ranges for this line
    let mut ranges: Vec<(usize, usize, HighlightType)> = Vec::new();

    // Add visual selection range if present
    if let (Some(start), Some(end)) =
      (self.editor_state.selection_start, self.editor_state.selection_end)
    {
      let is_line_mode =
        self.editor_state.mode == super::core::EditorMode::VisualLine;

      if is_line_mode
        && current_line_idx >= start.0.min(end.0)
        && current_line_idx <= start.0.max(end.0)
      {
        ranges.push((0, line.len(), HighlightType::Selection));
      } else if !is_line_mode {
        // Handle character mode selection
        if start.0 == end.0 && current_line_idx == start.0 {
          let start_col = start.1.min(end.1);
          let end_col = start.1.max(end.1).min(line.len());
          if start_col < end_col {
            ranges.push((start_col, end_col, HighlightType::Selection));
          }
        } else if current_line_idx >= start.0.min(end.0)
          && current_line_idx <= start.0.max(end.0)
        {
          // Multi-line selection logic
          if current_line_idx == start.0.min(end.0) {
            let col = if start.0 < end.0 { start.1 } else { end.1 };
            ranges.push((col, line.len(), HighlightType::Selection));
          } else if current_line_idx == start.0.max(end.0) {
            let col = if start.0 > end.0 { start.1 } else { end.1 };
            ranges.push((0, col.min(line.len()), HighlightType::Selection));
          } else {
            ranges.push((0, line.len(), HighlightType::Selection));
          }
        }
      }
    }

    // Add persistent highlight ranges
    let Some((abs_line_start, abs_line_end)) =
      Self::persistent_highlight_line_range(
        current_line_idx,
        buffer_lines,
        buffer_line_kinds,
      )
    else {
      return Ok(false);
    };

    let line_highlights =
      self.highlights.get_highlights_for_range(abs_line_start, abs_line_end);
    for highlight in line_highlights {
      let start = if highlight.start <= abs_line_start {
        0
      } else {
        highlight.start - abs_line_start
      };
      let end = if highlight.end >= abs_line_end {
        line.len()
      } else {
        highlight.end - abs_line_start
      };

      if end > start && start < line.len() {
        ranges.push((
          start.min(line.len()),
          end.min(line.len()),
          HighlightType::Persistent,
        ));
      }
    }

    if ranges.is_empty() {
      return Ok(false);
    }

    // Sort ranges by start position
    ranges.sort_by_key(|r| r.0);

    // Render the line with all highlights
    write!(stdout, "{center_offset_string}")?;
    let mut last_end = 0;

    for (start, end, highlight_type) in ranges {
      // Print unhighlighted text before this highlight
      if start > last_end {
        write!(stdout, "{}", &line[last_end..start])?;
      }

      // Print highlighted text with appropriate style
      match highlight_type {
        HighlightType::Selection => {
          execute!(
            stdout,
            SetBackgroundColor(Color::DarkBlue),
            SetForegroundColor(Color::White)
          )?;
        }
        HighlightType::Persistent => {
          execute!(
            stdout,
            SetBackgroundColor(Color::Yellow),
            SetForegroundColor(Color::Black)
          )?;
        }
      }

      // Handle overlapping ranges - use the max end
      let actual_end = if last_end > start { last_end.max(end) } else { end };
      let actual_start = last_end.max(start);

      if actual_start < actual_end && actual_start < line.len() {
        write!(stdout, "{}", &line[actual_start..actual_end.min(line.len())])?;
      }

      execute!(stdout, ResetColor)?;
      last_end = actual_end;
    }

    // Print remaining unhighlighted text
    if last_end < line.len() {
      write!(stdout, "{}", &line[last_end..])?;
    }

    // Clear to end of line to match normal view rendering
    execute!(stdout, Clear(ClearType::UntilNewLine))?;

    Ok(true)
  }

  // Buffered version of render_pane_combined_highlights
  #[allow(clippy::too_many_arguments)]
  fn render_pane_combined_highlights_buffered(
    &self,
    buffer: &mut Vec<u8>,
    buffer_idx: usize,
    viewport_line_idx: usize,
    line: &str,
    center_offset_string: &str,
    offset: usize,
    buffer_lines: &[String],
    buffer_line_kinds: &[PdfLineKind],
  ) -> IoResult<bool> {
    // Only main buffer has persistent highlights
    if buffer_idx != 0 {
      return self.render_pane_selection_buffered(
        buffer,
        buffer_idx,
        viewport_line_idx,
        line,
        center_offset_string,
      );
    }

    let current_line_idx = offset + viewport_line_idx;

    // Get all highlight ranges for this line
    let mut ranges: Vec<(usize, usize, HighlightType)> = Vec::new();

    // Add visual selection range if present
    if let (Some(start), Some(end)) =
      (self.editor_state.selection_start, self.editor_state.selection_end)
    {
      let is_line_mode =
        self.editor_state.mode == super::core::EditorMode::VisualLine;

      if is_line_mode
        && current_line_idx >= start.0.min(end.0)
        && current_line_idx <= start.0.max(end.0)
      {
        ranges.push((0, line.len(), HighlightType::Selection));
      } else if !is_line_mode {
        // Handle character mode selection
        if start.0 == end.0 && current_line_idx == start.0 {
          let start_col = start.1.min(end.1);
          let end_col = start.1.max(end.1).min(line.len());
          if start_col < end_col {
            ranges.push((start_col, end_col, HighlightType::Selection));
          }
        } else if current_line_idx >= start.0.min(end.0)
          && current_line_idx <= start.0.max(end.0)
        {
          // Multi-line selection logic
          if current_line_idx == start.0.min(end.0) {
            let col = if start.0 < end.0 { start.1 } else { end.1 };
            ranges.push((col, line.len(), HighlightType::Selection));
          } else if current_line_idx == start.0.max(end.0) {
            let col = if start.0 > end.0 { start.1 } else { end.1 };
            ranges.push((0, col.min(line.len()), HighlightType::Selection));
          } else {
            ranges.push((0, line.len(), HighlightType::Selection));
          }
        }
      }
    }

    // Add persistent highlight ranges
    let Some((abs_line_start, abs_line_end)) =
      Self::persistent_highlight_line_range(
        current_line_idx,
        buffer_lines,
        buffer_line_kinds,
      )
    else {
      return Ok(false);
    };

    let line_highlights =
      self.highlights.get_highlights_for_range(abs_line_start, abs_line_end);
    for highlight in line_highlights {
      let start = if highlight.start <= abs_line_start {
        0
      } else {
        highlight.start - abs_line_start
      };
      let end = if highlight.end >= abs_line_end {
        line.len()
      } else {
        highlight.end - abs_line_start
      };

      if end > start && start < line.len() {
        ranges.push((
          start.min(line.len()),
          end.min(line.len()),
          HighlightType::Persistent,
        ));
      }
    }

    if ranges.is_empty() {
      return Ok(false);
    }

    // Sort ranges by start position
    ranges.sort_by_key(|r| r.0);

    // Render the line with all highlights
    write!(buffer, "{center_offset_string}")?;
    let mut last_end = 0;

    for (start, end, highlight_type) in ranges {
      // Print unhighlighted text before this highlight
      if start > last_end {
        write!(buffer, "{}", &line[last_end..start])?;
      }

      // Print highlighted text with appropriate style
      match highlight_type {
        HighlightType::Selection => {
          buffer.queue(SetBackgroundColor(Color::DarkBlue))?;
          buffer.queue(SetForegroundColor(Color::White))?;
        }
        HighlightType::Persistent => {
          buffer.queue(SetBackgroundColor(Color::Yellow))?;
          buffer.queue(SetForegroundColor(Color::Black))?;
        }
      }

      // Handle overlapping ranges - use the max end
      let actual_end = if last_end > start { last_end.max(end) } else { end };
      let actual_start = last_end.max(start);

      if actual_start < actual_end && actual_start < line.len() {
        write!(buffer, "{}", &line[actual_start..actual_end.min(line.len())])?;
      }

      buffer.queue(ResetColor)?;
      last_end = actual_end;
    }

    // Print remaining unhighlighted text
    if last_end < line.len() {
      write!(buffer, "{}", &line[last_end..])?;
    }

    // Clear to end of line to match normal view rendering
    buffer.queue(Clear(ClearType::UntilNewLine))?;

    Ok(true)
  }

  // Render persistent highlights for a pane line
  #[allow(clippy::too_many_arguments)]
  fn render_pane_persistent_highlights(
    &self,
    stdout: &mut io::Stdout,
    buffer_idx: usize,
    viewport_line_idx: usize,
    line: &str,
    center_offset_string: &str,
    offset: usize,
    buffer_lines: &[String],
    buffer_line_kinds: &[PdfLineKind],
  ) -> IoResult<bool> {
    // Only main buffer has persistent highlights
    if buffer_idx != 0 {
      return Ok(false);
    }

    // Get the actual line index in the main buffer
    let current_line_idx = offset + viewport_line_idx;

    let Some((abs_line_start, abs_line_end)) =
      Self::persistent_highlight_line_range(
        current_line_idx,
        buffer_lines,
        buffer_line_kinds,
      )
    else {
      return Ok(false);
    };

    // Get highlights that overlap with this line
    let line_highlights =
      self.highlights.get_highlights_for_range(abs_line_start, abs_line_end);

    if line_highlights.is_empty() {
      return Ok(false);
    }

    // Convert highlights to line-relative positions and merge overlapping
    // ranges
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for highlight in line_highlights {
      let start = if highlight.start <= abs_line_start {
        0
      } else {
        highlight.start - abs_line_start
      };

      let end = if highlight.end >= abs_line_end {
        line.len()
      } else {
        highlight.end - abs_line_start
      };

      if end > start && start < line.len() {
        ranges.push((start.min(line.len()), end.min(line.len())));
      }
    }

    if ranges.is_empty() {
      return Ok(false);
    }

    // Sort and merge overlapping ranges
    ranges.sort_by_key(|r| r.0);
    let mut merged_ranges: Vec<(usize, usize)> = Vec::new();
    for range in ranges {
      if let Some(last) = merged_ranges.last_mut() {
        if range.0 <= last.1 {
          // Overlapping or adjacent, merge
          last.1 = last.1.max(range.1);
        } else {
          merged_ranges.push(range);
        }
      } else {
        merged_ranges.push(range);
      }
    }

    // Render the line with highlights

    write!(stdout, "{center_offset_string}")?;
    let mut last_end = 0;

    for (start, end) in merged_ranges {
      // Print unhighlighted text before this highlight
      if start > last_end {
        write!(stdout, "{}", &line[last_end..start])?;
      }

      // Print highlighted text
      execute!(
        stdout,
        SetBackgroundColor(Color::Yellow),
        SetForegroundColor(Color::Black)
      )?;
      write!(stdout, "{}", &line[start..end])?;
      execute!(stdout, ResetColor)?;

      last_end = end;
    }

    // Print remaining unhighlighted text
    if last_end < line.len() {
      write!(stdout, "{}", &line[last_end..])?;
    }

    // Clear to end of line to match normal view rendering
    execute!(stdout, Clear(ClearType::UntilNewLine))?;

    Ok(true)
  }

  // Buffered version of render_pane_persistent_highlights
  #[allow(clippy::too_many_arguments)]
  fn render_pane_persistent_highlights_buffered(
    &self,
    buffer: &mut Vec<u8>,
    buffer_idx: usize,
    viewport_line_idx: usize,
    line: &str,
    center_offset_string: &str,
    offset: usize,
    buffer_lines: &[String],
    buffer_line_kinds: &[PdfLineKind],
  ) -> IoResult<bool> {
    // Only main buffer has persistent highlights
    if buffer_idx != 0 {
      return Ok(false);
    }

    // Get the actual line index in the main buffer
    let current_line_idx = offset + viewport_line_idx;

    let Some((abs_line_start, abs_line_end)) =
      Self::persistent_highlight_line_range(
        current_line_idx,
        buffer_lines,
        buffer_line_kinds,
      )
    else {
      return Ok(false);
    };

    // Get highlights that overlap with this line
    let line_highlights =
      self.highlights.get_highlights_for_range(abs_line_start, abs_line_end);

    if line_highlights.is_empty() {
      return Ok(false);
    }

    // Convert highlights to line-relative positions and merge overlapping
    // ranges
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for highlight in line_highlights {
      let start = if highlight.start <= abs_line_start {
        0
      } else {
        highlight.start - abs_line_start
      };

      let end = if highlight.end >= abs_line_end {
        line.len()
      } else {
        highlight.end - abs_line_start
      };

      if end > start && start < line.len() {
        ranges.push((start.min(line.len()), end.min(line.len())));
      }
    }

    if ranges.is_empty() {
      return Ok(false);
    }

    // Sort and merge overlapping ranges
    ranges.sort_by_key(|r| r.0);
    let mut merged_ranges: Vec<(usize, usize)> = Vec::new();
    for range in ranges {
      if let Some(last) = merged_ranges.last_mut() {
        if range.0 <= last.1 {
          // Overlapping or adjacent, merge
          last.1 = last.1.max(range.1);
        } else {
          merged_ranges.push(range);
        }
      } else {
        merged_ranges.push(range);
      }
    }

    // Render the line with highlights
    write!(buffer, "{center_offset_string}")?;
    let mut last_end = 0;

    for (start, end) in merged_ranges {
      // Print unhighlighted text before this highlight
      if start > last_end {
        write!(buffer, "{}", &line[last_end..start])?;
      }

      // Print highlighted text
      buffer.queue(SetBackgroundColor(Color::Yellow))?;
      buffer.queue(SetForegroundColor(Color::Black))?;
      write!(buffer, "{}", &line[start..end])?;
      buffer.queue(ResetColor)?;

      last_end = end;
    }

    // Print remaining unhighlighted text
    if last_end < line.len() {
      write!(buffer, "{}", &line[last_end..])?;
    }

    // Clear to end of line to match normal view rendering
    buffer.queue(Clear(ClearType::UntilNewLine))?;

    Ok(true)
  }

  // Check if a line in a pane has visual selection
  fn has_pane_selection_on_line(
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
  fn render_pane_selection(
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
              self.editor_state.mode == super::core::EditorMode::VisualLine,
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
              buffer.mode == super::core::EditorMode::VisualLine,
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
  fn render_pane_selection_buffered(
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
              self.editor_state.mode == super::core::EditorMode::VisualLine,
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
              buffer.mode == super::core::EditorMode::VisualLine,
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
