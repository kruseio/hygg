use crossterm::{cursor::MoveTo, execute};
use std::io::{self, Write};

use super::super::core::Editor;
use super::layout::{StatusSlot, draw_right_anchored};

pub(crate) const PDF_LOADING_SLOT_WIDTH: usize = 14; // "P[x] O[x] T[x]"
// Blank columns kept to the right of the status bar so it never touches the
// terminal edge.
const PROGRESS_RIGHT_MARGIN: usize = 2;
// Blank columns between adjacent status-bar slots.
const STATUS_SLOT_GAP: usize = 1;
pub(crate) const PDF_LOADING_FRAMES: [&str; 4] = ["◰", "◳", "◲", "◱"];

/// Reading position / page counter, drawn at the right edge of the status bar.
struct ProgressSlot<'a> {
  editor: &'a Editor,
}

impl StatusSlot for ProgressSlot<'_> {
  fn reserved_width(&self) -> usize {
    self.editor.progress_indicator_slot_width()
  }

  fn render(&self) -> Option<String> {
    // Reserved even when hidden (so the loading slot beside it stays put), but
    // only drawn when progress display is on.
    self.editor.show_progress.then(|| self.editor.progress_indicator_message())
  }
}

/// PDF parser / OCR / TTS loading spinners, drawn just left of the progress
/// slot. Always reserves its full width so the progress slot never shifts as
/// spinners come and go.
struct PdfLoadingSlot<'a> {
  editor: &'a Editor,
}

impl StatusSlot for PdfLoadingSlot<'_> {
  fn reserved_width(&self) -> usize {
    PDF_LOADING_SLOT_WIDTH
  }

  fn render(&self) -> Option<String> {
    Some(self.editor.pdf_loading_slots_message())
  }
}

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

  /// Draw the right-anchored status bar (loading spinners + progress / page
  /// counter). Works on any writer, so the immediate and buffered render paths
  /// share one implementation. Slots are listed rightmost-first; each declares
  /// its reserved width via `StatusSlot`, so adding a future element is just a
  /// matter of implementing the trait and inserting it here.
  pub(crate) fn draw_status_bar<W: Write>(
    &self,
    out: &mut W,
  ) -> io::Result<()> {
    self.debug_log(&format!(
      "Drawing status bar: {} (view_mode: {:?}, demo: {})",
      self.progress_indicator_message(),
      self.view_mode,
      self.tutorial_demo_mode
    ));
    let progress = ProgressSlot { editor: self };
    let loading = PdfLoadingSlot { editor: self };
    draw_right_anchored(
      out,
      self.width,
      self.height as u16 - 2,
      PROGRESS_RIGHT_MARGIN,
      STATUS_SLOT_GAP,
      &[&progress, &loading],
    )
  }

  pub(crate) fn progress_indicator_message(&self) -> String {
    // The percentage is line-based, so it drifts while pages stream in and
    // `total_lines` keeps growing. Hold it at `--` until the parser finishes
    // rather than show a number that walks backwards.
    let percent = if self.progress_is_loading() {
      "--".to_string()
    } else {
      self.read_progress_percent().to_string()
    };

    // PDFs carry a physical page structure readers navigate by, so show the
    // absolute page alongside the percentage (e.g. `37/250 (18%)`). The page
    // counter is shown as soon as the page count is known — including while
    // the document is still streaming — so it never pops in late and shifts
    // the rest of the indicator. Flowed formats (EPUB, plain text, …) have no
    // fixed pages and fall back to the percentage alone.
    match self.current_page_indicator() {
      Some((page, total_pages)) => format!("{page}/{total_pages} ({percent}%)"),
      None => format!("{percent}%"),
    }
  }

  /// True while page content is still being parsed/streamed, so position- and
  /// percentage-derived values can't be trusted yet.
  fn progress_is_loading(&self) -> bool {
    self.pdf_pending.is_some()
      || self.pdf_streaming.as_ref().is_some_and(|s| !s.fully_loaded)
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
  /// is known, or `None` when the page counter is off or the format has no
  /// physical pages.
  fn current_page_indicator(&self) -> Option<(u32, usize)> {
    let total_pages = self.page_counter_total()?;
    let (page, _) = self.current_pdf_position()?;
    Some((page, total_pages))
  }

  /// Total page count to surface, or `None` when the page counter is disabled
  /// (the default) or the document has no physical pages. Page numbers are
  /// only available for PDFs, whose page table gives an authoritative count;
  /// flowed formats (EPUB, DOCX, plain text, …) have no fixed pages.
  fn page_counter_total(&self) -> Option<usize> {
    if !self.show_page_numbers {
      return None;
    }
    let total_pages = self.pdf_streaming.as_ref()?.pages.len();
    (total_pages > 0).then_some(total_pages)
  }

  /// Columns reserved for the progress indicator. When the page counter is
  /// shown this is held at the widest it can reach for the current document
  /// (`{total}/{total} (100%)`) so its left edge — and therefore the loading
  /// spinners drawn beside it — stay put as the page, percentage, and loading
  /// placeholder fill in. Otherwise it is just the percentage (`100%`).
  pub(crate) fn progress_indicator_slot_width(&self) -> usize {
    match self.page_counter_total() {
      Some(total) => format!("{total}/{total} (100%)").len(),
      None => "100%".len(),
    }
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
