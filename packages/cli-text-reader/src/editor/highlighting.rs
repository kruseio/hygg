use crossterm::{
  QueueableCommand,
  cursor::MoveTo,
  execute,
  style::{Color, ResetColor, SetBackgroundColor, SetForegroundColor},
};
use std::io::{Result as IoResult, Write};

use super::core::Editor;

impl Editor {
  // Highlight current line
  pub fn highlight_current_line(
    &self,
    stdout: &mut impl Write,
    line_index: usize,
    term_width: u16,
  ) -> IoResult<bool> {
    // Suppress the highlight bar while the PDF is still being opened in
    // the background. cursor_y is already pre-positioned to the row the
    // streaming install will land on, so once pdf_pending clears the bar
    // appears in its final position without a jump.
    if self.pdf_pending.is_some() {
      return Ok(false);
    }
    if self.show_highlighter && line_index == self.cursor_y {
      self.debug_log(&format!(
        "Highlighting line {} with width {} (view_mode: {:?})",
        line_index, term_width, self.view_mode
      ));

      // First, draw the background for the entire line
      execute!(
        stdout,
        MoveTo(0, line_index as u16),
        SetBackgroundColor(Color::Rgb { r: 40, g: 40, b: 40 })
      )?;

      // Fill the entire width with background color
      write!(stdout, "{}", " ".repeat(term_width as usize))?;

      // Reset cursor position to beginning of line
      execute!(stdout, MoveTo(0, line_index as u16), ResetColor)?;

      Ok(true)
    } else {
      Ok(false)
    }
  }

  // Highlight search matches
  pub fn highlight_search_match(
    &self,
    stdout: &mut impl Write,
    line_index: usize,
    line: &str,
    center_offset_string: &str,
  ) -> IoResult<bool> {
    // Check for preview match first (during search mode)
    let match_to_highlight = if self.editor_state.search_preview_active {
      self.editor_state.search_preview_match
    } else {
      self.editor_state.current_match
    };

    if let Some((line_idx, start, end)) = match_to_highlight
      && line_idx == self.offset + line_index
    {
      write!(stdout, "{center_offset_string}")?;
      write!(stdout, "{}", &line[..start])?;
      execute!(
        stdout,
        SetBackgroundColor(Color::Yellow),
        SetForegroundColor(Color::Black)
      )?;
      write!(stdout, "{}", &line[start..end])?;
      execute!(stdout, ResetColor)?;
      write!(stdout, "{}", &line[end..])?;
      execute!(
        stdout,
        crossterm::terminal::Clear(
          crossterm::terminal::ClearType::UntilNewLine
        )
      )?;
      return Ok(true);
    }

    Ok(false)
  }

  // Check if a line has search match
  pub fn has_search_match_on_line(&self, line_index: usize) -> bool {
    // Check for preview match first (during search mode)
    let match_to_check = if self.editor_state.search_preview_active {
      self.editor_state.search_preview_match
    } else {
      self.editor_state.current_match
    };

    if let Some((line_idx, _, _)) = match_to_check {
      line_idx == self.offset + line_index
    } else {
      false
    }
  }

  // Buffered version of highlight_current_line
  pub fn highlight_current_line_buffered(
    &self,
    buffer: &mut Vec<u8>,
    line_index: usize,
    term_width: u16,
  ) -> IoResult<bool> {
    // Suppress the highlight bar while the PDF is still being opened in
    // the background. cursor_y is already pre-positioned to the row the
    // streaming install will land on, so once pdf_pending clears the bar
    // appears in its final position without a jump.
    if self.pdf_pending.is_some() {
      return Ok(false);
    }
    if self.show_highlighter && line_index == self.cursor_y {
      self.debug_log(&format!(
        "Highlighting line {} with width {} (view_mode: {:?})",
        line_index, term_width, self.view_mode
      ));

      // First, draw the background for the entire line
      buffer.queue(MoveTo(0, line_index as u16))?;
      buffer.queue(SetBackgroundColor(Color::Rgb { r: 40, g: 40, b: 40 }))?;

      // Fill the entire width with background color
      write!(buffer, "{}", " ".repeat(term_width as usize))?;

      // Reset cursor position to beginning of line
      buffer.queue(MoveTo(0, line_index as u16))?;
      buffer.queue(ResetColor)?;

      Ok(true)
    } else {
      Ok(false)
    }
  }

  // Buffered version of highlight_search_match
  pub fn highlight_search_match_buffered(
    &self,
    buffer: &mut Vec<u8>,
    line_index: usize,
    line: &str,
    center_offset_string: &str,
  ) -> IoResult<bool> {
    // Check for preview match first (during search mode)
    let match_to_highlight = if self.editor_state.search_preview_active {
      self.editor_state.search_preview_match
    } else {
      self.editor_state.current_match
    };

    if let Some((line_idx, start, end)) = match_to_highlight
      && line_idx == self.offset + line_index
    {
      write!(buffer, "{center_offset_string}")?;
      write!(buffer, "{}", &line[..start])?;
      buffer.queue(SetBackgroundColor(Color::Yellow))?;
      buffer.queue(SetForegroundColor(Color::Black))?;
      write!(buffer, "{}", &line[start..end])?;
      buffer.queue(ResetColor)?;
      write!(buffer, "{}", &line[end..])?;
      buffer.queue(crossterm::terminal::Clear(
        crossterm::terminal::ClearType::UntilNewLine,
      ))?;
      return Ok(true);
    }

    Ok(false)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn buffered_current_line_resets_after_background_setup() -> IoResult<()> {
    let mut editor = Editor::new(vec!["current".to_string()], 80);
    editor.cursor_y = 2;

    let mut buffer = Vec::new();
    assert!(editor.highlight_current_line_buffered(&mut buffer, 2, 4)?);

    let mut expected_tail = b"    ".to_vec();
    expected_tail.queue(MoveTo(0, 2))?;
    expected_tail.queue(ResetColor)?;

    assert!(
      buffer.ends_with(&expected_tail),
      "expected buffered highlight setup to fill the line, move back, then reset colors; got {:?}",
      String::from_utf8_lossy(&buffer)
    );

    Ok(())
  }
}
