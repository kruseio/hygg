use crossterm::{QueueableCommand, cursor::MoveTo, execute};
use std::io::{self, Write};

use super::super::core::{Editor, EditorMode};

impl Editor {
  // Draw the status line with mode indicators and position info
  pub fn draw_status_line(
    &mut self,
    stdout: &mut io::Stdout,
  ) -> io::Result<()> {
    // Draw mode indicators in the status line
    self.draw_mode_indicator(stdout)?;

    // Position info is now always hidden per user request

    if self.view_mode == super::super::core::ViewMode::Normal
      && !self.tutorial_demo_mode
    {
      if self.show_progress {
        self.draw_progress_indicator(stdout)?;
      } else {
        let y = self.height as u16 - 2;
        self.draw_pdf_loading_slots(
          stdout,
          self.progress_indicator_x() as u16,
          y,
        )?;
      }
    }

    Ok(())
  }

  // Draw mode indicator in the status line
  fn draw_mode_indicator(&mut self, stdout: &mut io::Stdout) -> io::Result<()> {
    // Always use the active buffer's mode - this ensures command line is shown
    // properly
    let effective_mode = self.get_active_mode();

    match effective_mode {
      EditorMode::Command => {
        execute!(stdout, MoveTo(0, (self.height - 1) as u16))?;
        let command = self.get_active_command_buffer();
        write!(stdout, ":{command}")?;
        self.draw_command_completion(stdout, 1 + command.len())?;
        execute!(
          stdout,
          crossterm::terminal::Clear(
            crossterm::terminal::ClearType::UntilNewLine
          )
        )?;
      }
      EditorMode::CommandExecution => {
        execute!(stdout, MoveTo(0, (self.height - 1) as u16))?;
        write!(stdout, ":{}", self.get_active_command_buffer())?;
        execute!(
          stdout,
          crossterm::terminal::Clear(
            crossterm::terminal::ClearType::UntilNewLine
          )
        )?;
      }
      EditorMode::Search => {
        execute!(stdout, MoveTo(0, (self.height - 1) as u16))?;
        write!(stdout, "/{}", self.get_active_command_buffer())?;
        execute!(
          stdout,
          crossterm::terminal::Clear(
            crossterm::terminal::ClearType::UntilNewLine
          )
        )?;
      }
      EditorMode::ReverseSearch => {
        execute!(stdout, MoveTo(0, (self.height - 1) as u16))?;
        write!(stdout, "?{}", self.get_active_command_buffer())?;
        execute!(
          stdout,
          crossterm::terminal::Clear(
            crossterm::terminal::ClearType::UntilNewLine
          )
        )?;
      }
      EditorMode::VisualChar => {
        execute!(stdout, MoveTo(0, (self.height - 1) as u16))?;
        write!(stdout, "-- VISUAL --")?;
        execute!(
          stdout,
          crossterm::terminal::Clear(
            crossterm::terminal::ClearType::UntilNewLine
          )
        )?;
      }
      EditorMode::VisualLine => {
        execute!(stdout, MoveTo(0, (self.height - 1) as u16))?;
        write!(stdout, "-- VISUAL LINE --")?;
        execute!(
          stdout,
          crossterm::terminal::Clear(
            crossterm::terminal::ClearType::UntilNewLine
          )
        )?;
      }
      EditorMode::Tutorial => {
        execute!(stdout, MoveTo(0, (self.height - 1) as u16))?;
        write!(stdout, "-- TUTORIAL --")?;
        execute!(
          stdout,
          crossterm::terminal::Clear(
            crossterm::terminal::ClearType::UntilNewLine
          )
        )?;
      }
      _ => {
        // Normal mode: surface a narration failure here, else clear the line.
        execute!(stdout, MoveTo(0, (self.height - 1) as u16))?;
        if let Some(err) = self.tts_error_message() {
          write!(stdout, "⚠ narration: {err}")?;
          execute!(
            stdout,
            crossterm::terminal::Clear(
              crossterm::terminal::ClearType::UntilNewLine
            )
          )?;
        } else {
          execute!(
            stdout,
            crossterm::terminal::Clear(
              crossterm::terminal::ClearType::CurrentLine
            )
          )?;
        }
      }
    }
    Ok(())
  }

  // Buffered version of draw_status_line
  pub fn draw_status_line_buffered(
    &mut self,
    buffer: &mut Vec<u8>,
  ) -> io::Result<()> {
    // Draw mode indicators in the status line
    self.draw_mode_indicator_buffered(buffer)?;

    if self.view_mode == super::super::core::ViewMode::Normal
      && !self.tutorial_demo_mode
    {
      if self.show_progress {
        self.draw_progress_indicator_buffered(buffer)?;
      } else {
        let y = self.height as u16 - 2;
        self.draw_pdf_loading_slots_buffered(
          buffer,
          self.progress_indicator_x() as u16,
          y,
        )?;
      }
    }

    Ok(())
  }

  // Buffered version of draw_mode_indicator
  fn draw_mode_indicator_buffered(
    &mut self,
    buffer: &mut Vec<u8>,
  ) -> io::Result<()> {
    let effective_mode = self.get_active_mode();

    buffer.queue(MoveTo(0, (self.height - 1) as u16))?;

    match effective_mode {
      EditorMode::Command => {
        let command = self.get_active_command_buffer();
        write!(buffer, ":{command}")?;
        self.draw_command_completion_buffered(buffer, 1 + command.len())?;
      }
      EditorMode::CommandExecution => {
        write!(buffer, ":{}", self.get_active_command_buffer())?;
      }
      EditorMode::Search => {
        write!(buffer, "/{}", self.get_active_command_buffer())?;
      }
      EditorMode::ReverseSearch => {
        write!(buffer, "?{}", self.get_active_command_buffer())?;
      }
      EditorMode::VisualChar => {
        write!(buffer, "-- VISUAL --")?;
      }
      EditorMode::VisualLine => {
        write!(buffer, "-- VISUAL LINE --")?;
      }
      EditorMode::Tutorial => {
        write!(buffer, "-- TUTORIAL --")?;
      }
      _ => {
        // Normal mode: surface a narration failure here, else leave it blank.
        if let Some(err) = self.tts_error_message() {
          write!(buffer, "⚠ narration: {err}")?;
        }
      }
    }

    // Clear to end of line after any text
    buffer.queue(crossterm::terminal::Clear(
      crossterm::terminal::ClearType::UntilNewLine,
    ))?;

    Ok(())
  }
}
