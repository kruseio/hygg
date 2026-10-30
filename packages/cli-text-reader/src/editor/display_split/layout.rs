use crossterm::{
  cursor::MoveTo,
  queue,
  style::{Color, ResetColor, SetForegroundColor},
};
use std::io::{Result as IoResult, Write};

use super::super::core::Editor;

impl Editor {
  // Draw the split view: two panes and the separator between them. Writes to
  // any writer, like `draw_content`.
  pub(crate) fn draw_split_view(
    &self,
    out: &mut impl Write,
    term_width: u16,
    center_offset_string: &str,
  ) -> IoResult<()> {
    self.debug_log(&format!(
      "=== draw_split_view === Active pane: {}, Active buffer: {}",
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

    // In tutorial mode the tutorial (1) is on top and the command buffer (2)
    // below; otherwise the main buffer (0) over the command buffer (1)
    let (top_buffer_idx, bottom_buffer_idx) =
      if self.tutorial_active && self.buffers.len() > 2 {
        (1, 2)
      } else {
        (0, 1)
      };

    self.draw_pane(out, top_buffer_idx, 0, top_height, center_offset_string)?;

    // Draw separator
    queue!(
      out,
      MoveTo(0, top_height as u16),
      SetForegroundColor(Color::DarkGrey)
    )?;
    write!(out, "{}", "─".repeat(term_width as usize))?;
    queue!(out, ResetColor)?;

    self.draw_pane(
      out,
      bottom_buffer_idx,
      top_height + 1, // after the separator
      bottom_height,
      center_offset_string,
    )
  }

  // The terminal frame's entry point (`display_loop`): same drawing, into the
  // frame buffer.
  pub(crate) fn draw_split_view_buffered(
    &self,
    buffer: &mut Vec<u8>,
    term_width: u16,
    center_offset_string: &str,
  ) -> IoResult<()> {
    self.draw_split_view(buffer, term_width, center_offset_string)
  }
}
