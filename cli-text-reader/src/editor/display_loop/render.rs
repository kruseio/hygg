use crossterm::{
  cursor::Hide,
  terminal::{
    BeginSynchronizedUpdate, Clear, ClearType, EndSynchronizedUpdate,
  },
};
use std::io::{self, IsTerminal, Result as IoResult, Write};

use super::super::core::{Editor, EditorMode, ViewMode};

impl Editor {
  pub(crate) fn show_idle_cursor_if_needed(
    &mut self,
    buffer: &mut Vec<u8>,
  ) -> IoResult<()> {
    if self.show_cursor
      && self.pdf_pending.is_none()
      && !self.cursor_currently_visible
    {
      use crossterm::QueueableCommand;

      buffer.queue(crossterm::cursor::Show)?;
      self.cursor_currently_visible = true;
    }
    Ok(())
  }

  /// Render one frame when a redraw is needed, or just keep the idle cursor
  /// positioned otherwise. Mirrors the original inline redraw block.
  pub(crate) fn render_frame(
    &mut self,
    stdout: &mut io::Stdout,
    first_iteration: bool,
    skip_first_center: bool,
  ) -> Result<(), Box<dyn std::error::Error>> {
    // Only redraw if needed
    if self.check_needs_redraw() || first_iteration {
      if std::io::stdout().is_terminal() {
        // Create a buffer to collect all rendering commands
        let mut render_buffer = Vec::new();

        // Only hide cursor if it's currently visible
        // This reduces flicker on Windows terminals
        use crossterm::QueueableCommand;
        render_buffer.queue(BeginSynchronizedUpdate)?;
        if self.cursor_currently_visible && self.show_cursor {
          render_buffer.queue(Hide)?;
          self.cursor_currently_visible = false;
        }

        // Only clear screen if forced or on first iteration
        if self.force_clear || first_iteration {
          render_buffer.queue(Clear(ClearType::All))?;
          self.force_clear = false;
        }

        // Center the cursor consistently - this will ensure the
        // cursor stays in the middle with equal lines above and below
        // Skip on first iteration if we loaded progress to preserve exact
        // position
        // Also skip centering when entering command/search modes to prevent
        // layout shift
        let should_skip_center = first_iteration && skip_first_center;
        let is_mode_change_only = matches!(
          self.editor_state.mode,
          EditorMode::Command
            | EditorMode::CommandExecution
            | EditorMode::Search
            | EditorMode::ReverseSearch
        ) && !self.cursor_moved;

        // Skip centering if we just switched buffers or demo hint is active
        let skip_for_demo_hint =
          self.tutorial_demo_mode && self.demo_hint_text.is_some();
        if !should_skip_center
          && !is_mode_change_only
          && !self.buffer_just_switched
          && !skip_for_demo_hint
        {
          self.center_cursor();
        }

        // Clear the buffer switch flag after checking
        if self.buffer_just_switched {
          self.debug_log("Skipping center_cursor due to buffer switch");
          self.buffer_just_switched = false;
        }

        // Calculate layout parameters
        let term_width = self.width as u16;
        let center_offset = if self.width > self.col {
          (self.width / 2) - self.col / 2
        } else {
          0
        };
        let center_offset_string = " ".repeat(center_offset);

        // Draw content based on view mode
        self.debug_log(&format!(
          "Drawing buffer {} in {:?} mode",
          self.active_buffer, self.view_mode
        ));

        // Draw all content to the buffer instead of stdout
        match self.view_mode {
          ViewMode::Normal | ViewMode::Overlay => {
            self.draw_content_buffered(
              &mut render_buffer,
              term_width,
              &center_offset_string,
            )?;
          }
          ViewMode::HorizontalSplit => {
            self.draw_split_view_buffered(
              &mut render_buffer,
              term_width,
              &center_offset_string,
            )?;
          }
        }

        // Show status line and position info
        self.draw_status_line_buffered(&mut render_buffer)?;

        // Render demo hint if active
        if self.tutorial_demo_mode {
          self.render_demo_hint_buffered(
            &mut render_buffer,
            self.width,
            self.height,
          )?;
        }

        // Position cursor and show it at the final position
        self.position_cursor_buffered(&mut render_buffer, center_offset)?;
        render_buffer.queue(EndSynchronizedUpdate)?;

        // Write everything to stdout in one go
        stdout.write_all(&render_buffer)?;
        stdout.flush()?;

        // Reset cursor_moved flag after rendering
        self.cursor_moved = false;
      } else {
        // Non-terminal rendering (keep original behavior)
        // Center the cursor consistently
        let should_skip_center = first_iteration && skip_first_center;
        let is_mode_change_only = matches!(
          self.editor_state.mode,
          EditorMode::Command
            | EditorMode::CommandExecution
            | EditorMode::Search
            | EditorMode::ReverseSearch
        ) && !self.cursor_moved;

        // Skip centering if we just switched buffers or demo hint is active
        let skip_for_demo_hint =
          self.tutorial_demo_mode && self.demo_hint_text.is_some();
        if !should_skip_center
          && !is_mode_change_only
          && !self.buffer_just_switched
          && !skip_for_demo_hint
        {
          self.center_cursor();
        }

        // Clear the buffer switch flag after checking
        if self.buffer_just_switched {
          self.debug_log(
            "Skipping center_cursor due to buffer switch (non-terminal)",
          );
          self.buffer_just_switched = false;
        }

        // Calculate layout parameters
        // Use cached `self.width` everywhere so the highlight-bar fill and
        // `center_offset` agree on the same width within one frame.
        let term_width = self.width as u16;
        let center_offset = if self.width > self.col {
          (self.width / 2) - self.col / 2
        } else {
          0
        };
        let center_offset_string = " ".repeat(center_offset);

        // Draw content based on view mode
        match self.view_mode {
          ViewMode::Normal | ViewMode::Overlay => {
            self.draw_content(stdout, term_width, &center_offset_string)?;
          }
          ViewMode::HorizontalSplit => {
            self.draw_split_view(stdout, term_width, &center_offset_string)?;
          }
        }

        // Show status line and position info
        self.draw_status_line(stdout)?;

        // Render demo hint if active
        if self.tutorial_demo_mode {
          self.render_demo_hint(stdout, self.width, self.height)?;
        }

        stdout.flush()?;
        self.cursor_moved = false;
      }
    } else {
      // Even if not redrawing, ensure cursor is visible and positioned
      // correctly But do it efficiently with a single write
      if std::io::stdout().is_terminal() {
        let mut buffer = Vec::new();
        self.show_idle_cursor_if_needed(&mut buffer)?;
        if !buffer.is_empty() {
          stdout.write_all(&buffer)?;
          stdout.flush()?;
        }
      }
    }
    Ok(())
  }
}
