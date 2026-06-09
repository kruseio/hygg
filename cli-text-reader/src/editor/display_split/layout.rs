use crossterm::{
  QueueableCommand, execute,
  style::{Color, ResetColor, SetForegroundColor},
};
use std::io::{self, Result as IoResult, Write};

use super::super::core::Editor;

impl Editor {
  // Draw split view with two panes
  pub(crate) fn draw_split_view(
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

  // Buffered version of draw_split_view
  pub(crate) fn draw_split_view_buffered(
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
}
