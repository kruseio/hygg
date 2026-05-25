use crossterm::{QueueableCommand, cursor::MoveTo, execute};
use std::io::{self, Write};

use super::core::{Editor, EditorMode};

const PROGRESS_SLOT_WIDTH: usize = 4;
const PDF_LOADING_SLOT_WIDTH: usize = 9;
const PDF_LOADING_FRAMES: [&str; 4] = ["◰", "◳", "◲", "◱"];

impl Editor {
  // Draw the status line with mode indicators and position info
  pub fn draw_status_line(
    &mut self,
    stdout: &mut io::Stdout,
  ) -> io::Result<()> {
    // Draw mode indicators in the status line
    self.draw_mode_indicator(stdout)?;

    // Position info is now always hidden per user request

    // Show progress indicator if enabled, in normal view mode, and not in demo.
    // PDF loading indicators use reserved slots immediately to the left, so
    // the progress percentage remains anchored while parser/OCR work comes
    // and goes.
    if self.show_progress
      && self.view_mode == super::core::ViewMode::Normal
      && !self.tutorial_demo_mode
    {
      self.draw_progress_indicator(stdout)?;
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
        write!(stdout, ":{}", self.get_active_command_buffer())?;
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
        // Clear the command line in normal mode
        execute!(stdout, MoveTo(0, (self.height - 1) as u16))?;
        execute!(
          stdout,
          crossterm::terminal::Clear(
            crossterm::terminal::ClearType::CurrentLine
          )
        )?;
      }
    }
    Ok(())
  }

  // Draw position information in the status line
  #[allow(dead_code)]
  fn draw_position_info(&self, stdout: &mut io::Stdout) -> io::Result<()> {
    let current_line = self.offset + self.cursor_y;

    // Add overlay indicator if we're in overlay mode
    let overlay_info = if self.view_mode == super::core::ViewMode::Overlay {
      if let Some(buffer) = self.buffers.get(1) {
        if let Some(cmd) = &buffer.command {
          format!(" [Overlay: {cmd}]  ")
        } else {
          " [Overlay]  ".to_string()
        }
      } else {
        String::new()
      }
    } else {
      String::new()
    };

    let position_info = format!(
      "{}{}: {} ({}/{})",
      overlay_info,
      current_line + 1,
      self.cursor_x + 1,
      current_line + 1,
      self.total_lines
    );

    let x = self.width as u16 - position_info.len() as u16 - 1;
    let y = self.height as u16 - 1;
    execute!(stdout, MoveTo(x, y))?;
    write!(stdout, "{position_info}")?;
    execute!(
      stdout,
      crossterm::terminal::Clear(crossterm::terminal::ClearType::UntilNewLine)
    )?;

    Ok(())
  }

  // Draw progress indicator in the status line area
  fn draw_progress_indicator(&self, stdout: &mut io::Stdout) -> io::Result<()> {
    let message = self.progress_indicator_message();

    self.debug_log(&format!(
      "Drawing progress indicator: {} (view_mode: {:?}, demo: {})",
      message, self.view_mode, self.tutorial_demo_mode
    ));
    let x = self.progress_indicator_x() as u16;
    let y = self.height as u16 - 2;
    self.draw_pdf_loading_slots(stdout, x, y)?;
    execute!(stdout, MoveTo(x, y))?;
    write!(stdout, "{message}")?;
    execute!(
      stdout,
      crossterm::terminal::Clear(crossterm::terminal::ClearType::UntilNewLine)
    )?;

    Ok(())
  }

  // Buffered version of draw_status_line
  pub fn draw_status_line_buffered(
    &mut self,
    buffer: &mut Vec<u8>,
  ) -> io::Result<()> {
    // Draw mode indicators in the status line
    self.draw_mode_indicator_buffered(buffer)?;

    // Show progress indicator if enabled, in normal view mode, and not in demo.
    // PDF loading indicators use reserved slots immediately to the left, so
    // the progress percentage remains anchored while parser/OCR work comes
    // and goes.
    if self.show_progress
      && self.view_mode == super::core::ViewMode::Normal
      && !self.tutorial_demo_mode
    {
      self.draw_progress_indicator_buffered(buffer)?;
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
        write!(buffer, ":{}", self.get_active_command_buffer())?;
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
        // Normal mode - just clear the line
      }
    }

    // Clear to end of line after any text
    buffer.queue(crossterm::terminal::Clear(
      crossterm::terminal::ClearType::UntilNewLine,
    ))?;

    Ok(())
  }

  // Buffered version of draw_progress_indicator
  fn draw_progress_indicator_buffered(
    &self,
    buffer: &mut Vec<u8>,
  ) -> io::Result<()> {
    let message = self.progress_indicator_message();

    self.debug_log(&format!(
      "Drawing progress indicator: {} (view_mode: {:?}, demo: {})",
      message, self.view_mode, self.tutorial_demo_mode
    ));
    let x = self.progress_indicator_x() as u16;
    let y = self.height as u16 - 2;
    self.draw_pdf_loading_slots_buffered(buffer, x, y)?;
    buffer.queue(MoveTo(x, y))?;
    write!(buffer, "{message}")?;
    buffer.queue(crossterm::terminal::Clear(
      crossterm::terminal::ClearType::UntilNewLine,
    ))?;

    Ok(())
  }

  fn progress_indicator_message(&self) -> String {
    if self.pdf_pending.is_some() {
      return format!("{:>width$}", "--%", width = PROGRESS_SLOT_WIDTH);
    }

    // Calculate actual position in document (offset + cursor position + 1 for
    // 1-based indexing)
    let current_position =
      (self.offset + self.cursor_y + 1).min(self.total_lines);
    let progress = if self.total_lines > 0 {
      (current_position as f64 / self.total_lines as f64 * 100.0)
        .round()
        .min(100.0)
    } else {
      100.0 // Empty document is 100% read
    };
    format!("{:>width$}", format!("{progress}%"), width = PROGRESS_SLOT_WIDTH)
  }

  fn progress_indicator_x(&self) -> usize {
    self.width.saturating_sub(PROGRESS_SLOT_WIDTH).saturating_sub(2)
  }

  fn pdf_loading_slots_message(&self) -> String {
    let parser_loading = self.pdf_pending.is_some()
      || self.pdf_streaming.as_ref().is_some_and(|s| !s.fully_loaded);
    let ocr_loading =
      self.pdf_streaming.as_ref().is_some_and(|s| s.ocr_loading);
    if !parser_loading && !ocr_loading {
      return " ".repeat(PDF_LOADING_SLOT_WIDTH);
    }

    let elapsed_ms = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .map(|duration| duration.as_millis())
      .unwrap_or(0);
    let frame_idx =
      (elapsed_ms / 120 % PDF_LOADING_FRAMES.len() as u128) as usize;
    pdf_loading_slots_message_for_state(parser_loading, ocr_loading, frame_idx)
  }

  fn pdf_loading_slots_x(&self) -> usize {
    self.progress_indicator_x().saturating_sub(PDF_LOADING_SLOT_WIDTH + 1)
  }

  fn draw_pdf_loading_slots(
    &self,
    stdout: &mut io::Stdout,
    progress_x: u16,
    y: u16,
  ) -> io::Result<()> {
    let x = self.pdf_loading_slots_x().min(progress_x as usize) as u16;
    let message = self.pdf_loading_slots_message();
    execute!(stdout, MoveTo(x, y))?;
    write!(stdout, "{message}")?;
    Ok(())
  }

  fn draw_pdf_loading_slots_buffered(
    &self,
    buffer: &mut Vec<u8>,
    progress_x: u16,
    y: u16,
  ) -> io::Result<()> {
    let x = self.pdf_loading_slots_x().min(progress_x as usize) as u16;
    let message = self.pdf_loading_slots_message();
    buffer.queue(MoveTo(x, y))?;
    write!(buffer, "{message}")?;
    Ok(())
  }
}

fn pdf_loading_slots_message_for_state(
  parser_loading: bool,
  ocr_loading: bool,
  frame_idx: usize,
) -> String {
  if !parser_loading && !ocr_loading {
    return " ".repeat(PDF_LOADING_SLOT_WIDTH);
  }

  let parser = if parser_loading {
    format!("P[{}]", PDF_LOADING_FRAMES[frame_idx % PDF_LOADING_FRAMES.len()])
  } else {
    "    ".to_string()
  };
  let ocr = if ocr_loading {
    format!(
      "O[{}]",
      PDF_LOADING_FRAMES[(frame_idx + 2) % PDF_LOADING_FRAMES.len()]
    )
  } else {
    "    ".to_string()
  };
  format!("{parser} {ocr}")
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn pdf_loading_slots_keep_fixed_width_when_inactive() {
    let message = pdf_loading_slots_message_for_state(false, false, 0);

    assert_eq!(message, " ".repeat(PDF_LOADING_SLOT_WIDTH));
    assert_eq!(message.chars().count(), PDF_LOADING_SLOT_WIDTH);
  }

  #[test]
  fn pdf_loading_slots_hide_completed_parser_slot() {
    let message = pdf_loading_slots_message_for_state(false, true, 0);

    assert_eq!(message, "     O[◲]");
    assert_eq!(message.chars().count(), PDF_LOADING_SLOT_WIDTH);
  }

  #[test]
  fn pdf_loading_slots_hide_completed_ocr_slot() {
    let message = pdf_loading_slots_message_for_state(true, false, 0);

    assert_eq!(message, "P[◰]     ");
    assert_eq!(message.chars().count(), PDF_LOADING_SLOT_WIDTH);
  }
}
