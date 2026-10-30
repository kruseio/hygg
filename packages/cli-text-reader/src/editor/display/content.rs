use crossterm::{
  queue,
  style::{Color, ResetColor, SetBackgroundColor, SetForegroundColor},
  terminal::{Clear, ClearType},
};
use std::io::{Result as IoResult, Write};

use super::super::core::Editor;
use super::super::highlighting::LineView;

impl Editor {
  // Draw the single-pane view's text rows. Writes to any writer: the terminal
  // path renders into a frame buffer (`draw_content_buffered`), the
  // non-terminal path straight to stdout and flushes once at the end.
  pub(crate) fn draw_content(
    &self,
    out: &mut impl Write,
    term_width: u16,
    center_offset_string: &str,
  ) -> IoResult<()> {
    let content_height = self.height.saturating_sub(1);

    for i in 0..content_height {
      queue!(out, crossterm::cursor::MoveTo(0, i as u16))?;

      // Calculate the actual line index in the document
      let line_idx = self.offset + i;
      let line = self.lines.get(line_idx);

      if let Some(line) = line
        && self.is_ansi_art_line(line_idx)
      {
        write!(out, "{center_offset_string}{line}")?;
        queue!(out, ResetColor, Clear(ClearType::UntilNewLine))?;
        continue;
      }

      // The cursor-line bar goes down first; the text is drawn over it
      let is_current_line = self.highlight_current_line(out, i, term_width)?;

      // Lines beyond the document (overscroll) are blank
      let text = line.map_or("", String::as_str);
      if line.is_some()
        && self.render_line_highlights(
          out,
          LineView::Main,
          line_idx,
          text,
          center_offset_string,
        )?
      {
        continue;
      }

      if is_current_line {
        // Contrast with the bar, and don't clear it
        queue!(out, SetForegroundColor(Color::White))?;
        write!(out, "{center_offset_string}{text}")?;
        queue!(out, ResetColor)?;
      } else {
        write!(out, "{center_offset_string}{text}")?;
        queue!(out, Clear(ClearType::UntilNewLine))?;
      }
    }

    // Reset highlighting at the end of each frame
    if self.show_highlighter {
      queue!(out, SetBackgroundColor(Color::Reset))?;
    }

    Ok(())
  }

  // The terminal frame's entry point (`display_loop`): same drawing, into the
  // frame buffer.
  pub(crate) fn draw_content_buffered(
    &self,
    buffer: &mut Vec<u8>,
    term_width: u16,
    center_offset_string: &str,
  ) -> IoResult<()> {
    self.draw_content(buffer, term_width, center_offset_string)
  }
}
