use crossterm::{
  cursor::MoveTo,
  queue,
  style::ResetColor,
  terminal::{Clear, ClearType},
};
use std::io::{Result as IoResult, Write};

use super::super::core::Editor;
use super::super::highlighting::LineView;

impl Editor {
  // Draw one pane of the split view: `height` rows of `buffers[buffer_idx]`
  // from screen row `start_row`. The cursor-line bar is not drawn in split
  // view.
  pub(crate) fn draw_pane(
    &self,
    out: &mut impl Write,
    buffer_idx: usize,
    start_row: usize,
    height: usize,
    center_offset_string: &str,
  ) -> IoResult<()> {
    let Some(buffer) = self.buffers.get(buffer_idx) else {
      self.debug_log(&format!("Warning: Buffer {buffer_idx} not found"));
      for i in 0..height {
        queue!(
          out,
          MoveTo(0, (start_row + i) as u16),
          Clear(ClearType::CurrentLine)
        )?;
      }
      return Ok(());
    };

    // The active buffer's live viewport is on the editor; an inactive buffer
    // keeps its own
    let offset = if buffer_idx == self.active_buffer {
      self.offset
    } else {
      buffer.offset
    };
    self.debug_log(&format!(
      "Drawing pane - buffer: {buffer_idx}, start_row: {start_row}, height: {height}, offset: {offset}, lines: {}",
      buffer.lines.len()
    ));

    for i in 0..height {
      queue!(out, MoveTo(0, (start_row + i) as u16))?;

      let line_idx = offset + i;
      let Some(line) = buffer.lines.get(line_idx) else {
        queue!(out, Clear(ClearType::CurrentLine))?;
        continue;
      };

      if self.is_buffer_ansi_art_line(buffer_idx, line_idx) {
        write!(out, "{center_offset_string}{line}")?;
        queue!(out, ResetColor, Clear(ClearType::UntilNewLine))?;
      } else if !self.render_line_highlights(
        out,
        LineView::Pane(buffer_idx),
        line_idx,
        line,
        center_offset_string,
      )? {
        write!(out, "{center_offset_string}{line}")?;
        queue!(out, Clear(ClearType::UntilNewLine))?;
      }
    }

    Ok(())
  }
}
