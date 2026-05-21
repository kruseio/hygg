use crossterm::{
  cursor::Hide,
  event::{self, Event as CEvent},
  execute,
  terminal::{self, Clear, ClearType},
};
use std::io::{self, IsTerminal, Result as IoResult, Write};

use super::core::{Editor, EditorMode, ViewMode};
use crate::progress::save_progress_full;

impl Editor {
  pub fn main_loop(
    &mut self,
    stdout: &mut io::Stdout,
    skip_first_center: bool,
  ) -> Result<(), Box<dyn std::error::Error>> {
    let mut first_iteration = true;

    loop {
      // If the PDF is still being opened in the background, see if the
      // opener has finished and install the streaming state when it has.
      if self.pdf_pending.is_some() {
        let _ = self.poll_pending_pdf_stream();
      }
      // Drain any pages the background PDF loader has finished extracting
      // before we render. This keeps the page table in sync and triggers a
      // redraw if anything new arrived.
      if self.pdf_streaming.is_some() {
        let _ = self.drain_pdf_stream();
      }

      self.debug_log(&format!(
        "Main loop iteration - buffers: {}, active: {}, mode: {:?}",
        self.buffers.len(),
        self.active_buffer,
        self.view_mode
      ));
      self.debug_log(&format!(
        "  Editor mode: {:?}, command_buffer: '{}'",
        self.editor_state.mode, self.editor_state.command_buffer
      ));
      self.debug_log(&format!(
        "  Active buffer lines: {}, cursor: ({}, {}), offset: {}, needs_redraw: {}, cursor_moved: {}",
        self.lines.len(),
        self.cursor_x,
        self.cursor_y,
        self.offset,
        self.needs_redraw,
        self.cursor_moved
      ));

      // Only redraw if needed
      if self.check_needs_redraw() || first_iteration {
        if std::io::stdout().is_terminal() {
          // Create a buffer to collect all rendering commands
          let mut render_buffer = Vec::new();

          // Only hide cursor if it's currently visible
          // This reduces flicker on Windows terminals
          use crossterm::QueueableCommand;
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
          let term_width = terminal::size()?.0 as u16;
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
          let term_width = 80u16; // Default width for non-terminal
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
              self.draw_split_view(
                stdout,
                term_width,
                &center_offset_string,
              )?;
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
        if self.show_cursor && std::io::stdout().is_terminal() {
          use crossterm::QueueableCommand;
          let mut buffer = Vec::new();
          buffer.queue(crossterm::cursor::Show)?;
          stdout.write_all(&buffer)?;
          stdout.flush()?;
        }
      }

      first_iteration = false;
      self.initial_setup_complete = true;

      // Handle keyboard input
      if std::io::stdout().is_terminal() {
        self.debug_log("Waiting for keyboard event...");
        // Use longer timeout when idle to reduce CPU usage
        let streaming_active = self
          .pdf_streaming
          .as_ref()
          .map(|s| !s.fully_loaded)
          .unwrap_or(false);
        let pending_pdf = self.pdf_pending.is_some();
        let timeout = if self.needs_redraw
          || self.tutorial_demo_mode
          || streaming_active
          || pending_pdf
        {
          std::time::Duration::from_millis(16) // ~60fps when animating, in demo, streaming, or opening PDF
        } else {
          std::time::Duration::from_millis(250) // Slower when idle
        };

        // Check for demo script actions
        if self.tutorial_demo_mode {
          // Check if hint should be cleared
          if let Some(until) = self.demo_hint_until
            && std::time::Instant::now() > until
          {
            // Only mark dirty if we actually had hint text
            if self.demo_hint_text.is_some() {
              self.demo_hint_text = None;
              self.demo_hint_until = None;
              self.mark_dirty();
            } else {
              self.demo_hint_until = None;
            }
          }

          if let Some(key_event) = self.check_demo_progress() {
            // Simulate the key event
            self.debug_log(&format!("Demo injecting key event: {key_event:?}"));
            let exit = self.handle_event(key_event, stdout)?;
            if exit {
              self.debug_log("Exiting from demo action");
              break;
            }
            // handle_event will mark dirty if needed
            continue;
          }

          // Check immediately after demo progress - demo might have just
          // completed
          if self.should_exit_after_demo() {
            self.debug_log("Demo complete, exiting (immediate)");
            break;
          }
        }

        // Check if demo should exit (after demo completion)
        if self.should_exit_after_demo() {
          self.debug_log(&format!(
            "Should exit after demo check: tutorial_demo_mode={}, demo_script={:?}, demo_action_index={}",
            self.tutorial_demo_mode,
            self.demo_script.is_some(),
            self.demo_action_index
          ));
          self.debug_log("Demo complete, exiting");
          break;
        }

        if event::poll(timeout)? {
          match event::read()? {
            CEvent::Key(key_event) => {
              // On Windows, crossterm sends both Press and Release events
              // We only want to process Press events to avoid double input
              if key_event.kind != crossterm::event::KeyEventKind::Press {
                self.debug_log(&format!(
                  "Ignoring key event with kind: {:?} (only processing Press events)",
                  key_event.kind
                ));
                continue;
              }

              // Enhanced debug logging for Windows key events
              #[cfg(target_os = "windows")]
              {
                self.debug_log(&format!(
                  "Windows key event details - code: {:?}, modifiers: {:?}, kind: {:?}, state: {:?}",
                  key_event.code, key_event.modifiers, key_event.kind, key_event.state
                ));
              }

              // Get the active buffer's mode
              let active_mode = self.get_active_mode();
              self.debug_log(&format!(
                "Key event: {:?} kind: {:?} in mode {:?}",
                key_event, key_event.kind, active_mode
              ));
              self.debug_log(&format!(
                "  Processing in buffer {} of {}",
                self.active_buffer,
                self.buffers.len()
              ));
              self.debug_log(&format!(
                "  Handling {} mode event",
                match active_mode {
                  EditorMode::Normal => "Normal",
                  EditorMode::VisualChar | EditorMode::VisualLine => "Visual",
                  EditorMode::Search | EditorMode::ReverseSearch => "Search",
                  EditorMode::Command | EditorMode::CommandExecution =>
                    "Command",
                  EditorMode::Tutorial => "Tutorial",
                }
              ));
              let exit = self.handle_event(key_event, stdout)?;

              if exit {
                self.debug_log("Exiting main loop");
                break;
              }
              let new_mode = self.get_active_mode();
              self.debug_log(&format!(
                "  After event - mode: {:?}, active_buffer: {}",
                new_mode, self.active_buffer
              ));
              // Mark as needing redraw after handling any key event
              self.mark_dirty();
            }
            CEvent::Resize(w, h) => {
              self.debug_log(&format!("Resize event: {w}x{h}"));
              self.width = w as usize;
              self.height = h as usize;
              // Only recenter after resize if initial setup is complete
              // This prevents overriding loaded progress position
              if self.initial_setup_complete {
                self.center_cursor();
              }
              // Force full clear and redraw after resize
              self.force_clear = true;
              self.mark_dirty();
            }
            _ => {}
          }
        } else {
          // No event available, just continue without logging to avoid spam
          continue;
        }
      } else {
        // In demo mode, continue even if not a terminal
        if !self.tutorial_demo_mode {
          self.debug_log("Not a terminal - exiting main loop");
          break;
        }

        // For demo mode when not in terminal, still check demo progress
        if self.tutorial_demo_mode
          && let Some(key_event) = self.check_demo_progress()
        {
          self.debug_log(&format!(
            "Demo injecting key event (non-terminal): {key_event:?}"
          ));
          let exit = self.handle_event(key_event, stdout)?;
          if exit {
            self.debug_log("Exiting from demo action (non-terminal)");
            break;
          }
          // handle_event will mark dirty if needed
        }

        // Check if demo should exit
        if self.should_exit_after_demo() {
          self.debug_log("Demo complete, exiting (non-terminal)");
          break;
        }

        // Wait a bit and continue
        std::thread::sleep(std::time::Duration::from_millis(50));
      }

      // Save progress with exact viewport state
      let current_line = self.offset + self.cursor_y;
      if current_line != self.last_offset
        || self.offset != self.last_saved_viewport_offset
      {
        let (page, line_in_page) = match self.current_pdf_position() {
          Some((p, l)) => (Some(p), Some(l)),
          None => (None, None),
        };
        save_progress_full(
          self.document_hash,
          current_line,
          self.total_lines,
          Some(self.offset),
          Some(self.cursor_y),
          page,
          line_in_page,
        )?;
        self.last_offset = current_line;
        self.last_saved_viewport_offset = self.offset;
      }
      self.debug_log("Main loop iteration complete\n");
    }

    Ok(())
  }
}
