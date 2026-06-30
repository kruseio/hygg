use crossterm::{
  cursor::{Hide, Show},
  execute,
  terminal::{self, Clear, ClearType},
};
use std::io::{self, IsTerminal, Result as IoResult};

use super::core::{Editor, EditorMode, ViewMode};
use crate::bookmarks::load_bookmarks;
use crate::config::load_config;
use crate::highlights::load_highlights;
use crate::progress::load_progress;

impl Editor {
  pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
    let mut stdout = io::stdout();
    let config = load_config();

    self.show_highlighter = config.enable_line_highlighter.unwrap_or(true);
    self.show_cursor = config.show_cursor.unwrap_or(true);
    self.show_progress = config.show_progress.unwrap_or(true);
    self.show_page_numbers = config.show_page_numbers.unwrap_or(false);

    // Check if tutorial should be shown
    let tutorial_enabled = config.enable_tutorial.unwrap_or(true);
    let tutorial_shown = config.tutorial_shown.unwrap_or(false);

    // Load bookmarks
    if let Ok(bookmark_data) = load_bookmarks(self.document_hash) {
      self.marks = bookmark_data.marks;
    }

    // Load highlights
    match load_highlights(&self.document_hash.to_string()) {
      Ok(highlight_data) => {
        self.highlights = highlight_data;
        self.debug_log(&format!(
          "Loaded {} highlights",
          self.highlights.highlights.len()
        ));
      }
      Err(e) => {
        self.debug_log_error(&format!("Failed to load highlights: {e}"));
      }
    }

    // Tutorial will be shown automatically on first launch if enabled

    // Note: Even with empty lines, we should allow the editor to run
    // so users can access the tutorial with :tutorial command

    let mut skip_first_center = false;
    // While the PDF is being opened in the background, the buffer is just
    // a single-line splash. Restoring a saved (line, cursor_y) from a
    // prior session at this point would point at non-existent rows and
    // make the splash render badly. The streaming install path will
    // jump straight to the target page once the doc parse completes.
    if self.pdf_pending.is_some() {
      // Blank splash: lib.rs hands us a one-element buffer of just an
      // empty string and pre-sets cursor_y to the row the streaming
      // install will land on. We just zero the viewport offset and skip
      // load_progress (which would point at non-existent rows in the
      // single-line splash buffer); cursor_y is left as lib.rs configured
      // it.
      self.offset = 0;
      self.last_offset = 0;
      self.last_saved_viewport_offset = 0;
      skip_first_center = true;
      self.debug_log("PDF pending in background; using predicted cursor_y");
    } else {
      match load_progress(self.document_hash) {
        Ok(progress) => {
          // Check if we have new viewport information
          if let (Some(viewport_offset), Some(saved_cursor_y)) =
            (progress.viewport_offset, progress.cursor_y)
          {
            // Use exact saved viewport state
            self.offset = viewport_offset;
            self.cursor_y = saved_cursor_y;
            self.debug_log(&format!(
            "Restored exact viewport state: offset={viewport_offset}, cursor_y={saved_cursor_y}"
          ));
          } else {
            // Fallback to old logic for backward compatibility
            let saved_line = progress.offset;
            let content_height = self.height.saturating_sub(1);
            let center_y = content_height / 2;

            // Try to center the saved line on screen
            if saved_line < center_y {
              // Line is near the top, can't center fully
              self.offset = 0;
              self.cursor_y = saved_line;
            } else if saved_line >= self.total_lines.saturating_sub(center_y) {
              // Line is near the bottom
              if self.total_lines > content_height {
                self.offset = self.total_lines - content_height;
                self.cursor_y = saved_line - self.offset;
              } else {
                self.offset = 0;
                self.cursor_y = saved_line;
              }
            } else {
              // Normal case - center the saved line
              self.offset = saved_line.saturating_sub(center_y);
              self.cursor_y = center_y;
            }
            self.debug_log(&format!(
            "Using fallback progress logic: line={saved_line}, offset={}, cursor_y={}", 
            self.offset, self.cursor_y
          ));
          }

          // Update tracking fields
          self.last_offset = progress.offset;
          self.last_saved_viewport_offset = self.offset;
          skip_first_center = true;
        }
        Err(e) => {
          self.debug_log(&format!("No progress found: {e}"));
          self.offset = 0;
          // cursor_y is already initialized to height/2 in the constructor
        }
      };
    }

    if std::io::stdout().is_terminal() {
      execute!(stdout, terminal::EnterAlternateScreen, Hide)?;
      terminal::enable_raw_mode()?;
    }

    // Show tutorial on first launch or start demo mode
    if self.tutorial_demo_mode {
      let demo_id = self.demo_id.unwrap_or(0); // Default to marketing demo if no ID specified
      self.debug_log(&format!("Starting demo mode with ID: {demo_id}"));
      self.start_demo_mode(demo_id);
    } else if tutorial_enabled && !tutorial_shown && !self.tutorial_demo_mode {
      self.debug_log("Showing interactive tutorial for first-time user");
      self.show_interactive_tutorial()?;
    }

    self.main_loop(&mut stdout, skip_first_center)?;

    self.cleanup(&mut stdout)?;
    Ok(())
  }

  pub fn cleanup(
    &self,
    stdout: &mut io::Stdout,
  ) -> Result<(), Box<dyn std::error::Error>> {
    if std::io::stdout().is_terminal() {
      execute!(stdout, Show, terminal::LeaveAlternateScreen)?;
      terminal::disable_raw_mode()?;
    }
    Ok(())
  }
}
