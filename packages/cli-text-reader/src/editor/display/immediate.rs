use crossterm::{
  execute,
  style::{ResetColor, SetBackgroundColor},
  terminal::{Clear, ClearType},
};
use std::io::{self, Result as IoResult, Write};

use super::super::core::Editor;

impl Editor {
  // Draw content with proper highlighting
  pub(crate) fn draw_content(
    &self,
    stdout: &mut io::Stdout,
    term_width: u16,
    center_offset_string: &str,
  ) -> IoResult<()> {
    let content_height = self.height.saturating_sub(1);

    for i in 0..content_height {
      execute!(stdout, crossterm::cursor::MoveTo(0, i as u16))?;

      // Calculate the actual line index in the document
      let line_idx = self.offset + i;

      if line_idx < self.lines.len() {
        // We have a real line to display
        let line = self.lines[line_idx].clone();

        if self.is_ansi_art_line(line_idx) {
          write!(stdout, "{center_offset_string}{line}")?;
          execute!(stdout, ResetColor, Clear(ClearType::UntilNewLine))?;
          continue;
        }

        // Highlight the current line first
        let is_current_line =
          self.highlight_current_line(stdout, i, term_width)?;

        // Check if we need to render any special highlights
        let has_selection = self.has_selection_on_line(i);
        let has_search = self.has_search_match_on_line(i);
        let has_persistent = self.has_persistent_highlights_on_line(i);

        // If we have multiple types of highlights, use combined rendering
        if (has_search || has_selection) && has_persistent {
          // Render line with combined highlights
          if self.render_combined_highlights(
            stdout,
            i,
            &line,
            center_offset_string,
          )? {
            continue;
          }
        }

        // Try highlighting selection only
        if has_selection
          && self.highlight_selection(stdout, i, &line, center_offset_string)?
        {
          continue;
        }

        // Try highlighting search match only
        if has_search
          && self.highlight_search_match(
            stdout,
            i,
            &line,
            center_offset_string,
          )?
        {
          continue;
        }

        // Try highlighting persistent highlights only
        if has_persistent
          && self.highlight_persistent(
            stdout,
            i,
            &line,
            center_offset_string,
          )?
        {
          continue;
        }

        // Normal line rendering - if current line was highlighted,
        // we need to use appropriate text color
        if is_current_line {
          // For the highlighted line, use a color that contrasts with the
          // background
          execute!(
            stdout,
            crossterm::style::SetForegroundColor(
              crossterm::style::Color::White
            )
          )?;
          write!(stdout, "{center_offset_string}{line}")?;
          execute!(stdout, crossterm::style::ResetColor)?;
          // Don't clear the line since we want to keep the background color
        } else {
          write!(stdout, "{center_offset_string}{line}")?;
          // Clear to end of line to avoid artifacts
          execute!(
            stdout,
            crossterm::terminal::Clear(
              crossterm::terminal::ClearType::UntilNewLine
            )
          )?;
        }
      } else {
        // This is beyond the document - show blank line for overscroll
        // But still check if we need to highlight the cursor line
        let is_current_line =
          self.highlight_current_line(stdout, i, term_width)?;

        if is_current_line {
          // Show highlighted empty line for cursor position
          execute!(
            stdout,
            crossterm::style::SetForegroundColor(
              crossterm::style::Color::White
            )
          )?;
          write!(stdout, "{center_offset_string}")?;
          execute!(stdout, crossterm::style::ResetColor)?;
          // Don't clear the line since we want to keep the background color
        } else {
          // Just show blank line
          write!(stdout, "{center_offset_string}")?;
          // Clear to end of line
          execute!(
            stdout,
            crossterm::terminal::Clear(
              crossterm::terminal::ClearType::UntilNewLine
            )
          )?;
        }
      }
    }

    // Reset highlighting at the end of each frame
    if self.show_highlighter {
      execute!(stdout, SetBackgroundColor(crossterm::style::Color::Reset))?;
    }

    Ok(())
  }
}
