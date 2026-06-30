use crossterm::{QueueableCommand, cursor::MoveTo, execute};
use std::io::{self, Write};

use super::super::core::Editor;

pub(crate) const PDF_LOADING_SLOT_WIDTH: usize = 14; // "P[x] O[x] T[x]"
// Blank columns kept to the right of the progress indicator so it never
// touches the terminal edge.
const PROGRESS_RIGHT_MARGIN: usize = 2;
pub(crate) const PDF_LOADING_FRAMES: [&str; 4] = ["◰", "◳", "◲", "◱"];

impl Editor {
  // Draw position information in the status line
  #[allow(dead_code)]
  fn draw_position_info(&self, stdout: &mut io::Stdout) -> io::Result<()> {
    let current_line = self.offset + self.cursor_y;

    // Add overlay indicator if we're in overlay mode
    let overlay_info =
      if self.view_mode == super::super::core::ViewMode::Overlay {
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
  pub(crate) fn draw_progress_indicator(
    &self,
    stdout: &mut io::Stdout,
  ) -> io::Result<()> {
    let message = self.progress_indicator_message();

    self.debug_log(&format!(
      "Drawing progress indicator: {} (view_mode: {:?}, demo: {})",
      message, self.view_mode, self.tutorial_demo_mode
    ));
    let x = self.progress_indicator_x_for(message.chars().count()) as u16;
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

  // Buffered version of draw_progress_indicator
  pub(crate) fn draw_progress_indicator_buffered(
    &self,
    buffer: &mut Vec<u8>,
  ) -> io::Result<()> {
    let message = self.progress_indicator_message();

    self.debug_log(&format!(
      "Drawing progress indicator: {} (view_mode: {:?}, demo: {})",
      message, self.view_mode, self.tutorial_demo_mode
    ));
    let x = self.progress_indicator_x_for(message.chars().count()) as u16;
    let y = self.height as u16 - 2;
    self.draw_pdf_loading_slots_buffered(buffer, x, y)?;
    buffer.queue(MoveTo(x, y))?;
    write!(buffer, "{message}")?;
    buffer.queue(crossterm::terminal::Clear(
      crossterm::terminal::ClearType::UntilNewLine,
    ))?;

    Ok(())
  }

  pub(crate) fn progress_indicator_message(&self) -> String {
    if self.pdf_pending.is_some()
      || self.pdf_streaming.as_ref().is_some_and(|s| !s.fully_loaded)
    {
      return "--%".to_string();
    }

    let progress = self.read_progress_percent();

    // PDFs carry a physical page structure readers navigate by, so show the
    // absolute page alongside the percentage (e.g. `37/250 (18%)`). Flowed
    // formats (EPUB, plain text, …) have no fixed pages and fall back to the
    // percentage alone.
    match self.current_page_indicator() {
      Some((page, total_pages)) => {
        format!("{page}/{total_pages} ({progress}%)")
      }
      None => format!("{progress}%"),
    }
  }

  /// Reading progress as a whole-number percentage of lines consumed, clamped
  /// to `0..=100`. An empty document counts as fully read.
  fn read_progress_percent(&self) -> u32 {
    if self.total_lines == 0 {
      return 100;
    }
    // offset + cursor position, +1 for 1-based indexing.
    let current_position =
      (self.offset + self.cursor_y + 1).min(self.total_lines);
    ((current_position as f64 / self.total_lines as f64 * 100.0).round() as u32)
      .min(100)
  }

  /// `(current_page_1based, total_pages)` for a streaming PDF whose page table
  /// is known, or `None` for formats without physical pages.
  fn current_page_indicator(&self) -> Option<(u32, usize)> {
    let total_pages = self.pdf_streaming.as_ref()?.pages.len();
    if total_pages == 0 {
      return None;
    }
    let (page, _) = self.current_pdf_position()?;
    Some((page, total_pages))
  }

  pub(crate) fn progress_indicator_x(&self) -> usize {
    self.progress_indicator_x_for(
      self.progress_indicator_message().chars().count(),
    )
  }

  /// Right-anchor a message of `message_len` columns, leaving
  /// `PROGRESS_RIGHT_MARGIN` blank columns at the terminal edge.
  pub(crate) fn progress_indicator_x_for(&self, message_len: usize) -> usize {
    self.width.saturating_sub(message_len).saturating_sub(PROGRESS_RIGHT_MARGIN)
  }

  fn pdf_loading_slots_message(&self) -> String {
    let parser_loading = self.pdf_pending.is_some()
      || self.pdf_streaming.as_ref().is_some_and(|s| !s.fully_loaded);
    let ocr_loading =
      self.pdf_streaming.as_ref().is_some_and(|s| s.ocr_loading);
    let tts_loading = self.is_tts_preparing();
    if !parser_loading && !ocr_loading && !tts_loading {
      return " ".repeat(PDF_LOADING_SLOT_WIDTH);
    }

    let elapsed_ms = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .map(|duration| duration.as_millis())
      .unwrap_or(0);
    let frame_idx =
      (elapsed_ms / 120 % PDF_LOADING_FRAMES.len() as u128) as usize;
    pdf_loading_slots_message_for_state(
      parser_loading,
      ocr_loading,
      tts_loading,
      frame_idx,
    )
  }

  fn pdf_loading_slots_x(&self) -> usize {
    self.progress_indicator_x().saturating_sub(PDF_LOADING_SLOT_WIDTH + 1)
  }

  pub(crate) fn draw_pdf_loading_slots(
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

  pub(crate) fn draw_pdf_loading_slots_buffered(
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

pub(crate) fn pdf_loading_slots_message_for_state(
  parser_loading: bool,
  ocr_loading: bool,
  tts_loading: bool,
  frame_idx: usize,
) -> String {
  if !parser_loading && !ocr_loading && !tts_loading {
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
  let tts = if tts_loading {
    format!(
      "T[{}]",
      PDF_LOADING_FRAMES[(frame_idx + 1) % PDF_LOADING_FRAMES.len()]
    )
  } else {
    "    ".to_string()
  };
  format!("{parser} {ocr} {tts}")
}
