use crossterm::{
  cursor::Hide,
  event::{self, Event as CEvent},
  execute,
  terminal::{
    BeginSynchronizedUpdate, Clear, ClearType, EndSynchronizedUpdate,
  },
};
use std::io::{self, IsTerminal, Result as IoResult, Write};

use super::core::{Editor, EditorMode, ViewMode};
use crate::progress::save_progress_full;

const FAST_EVENT_POLL_MS: u64 = 16;
const PDF_LOADING_EVENT_POLL_MS: u64 = 120;
const IDLE_EVENT_POLL_MS: u64 = 250;

impl Editor {
  fn show_idle_cursor_if_needed(
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
        self.pdf_load_finished = None;
        let _ = self.poll_pending_pdf_stream();
        // Repaint each tick so the elapsed-time counter in the loading
        // splash actually advances while we wait on the opener thread.
        if self.pdf_pending.is_some() {
          self.mark_dirty();
        }
      }
      // Drain any pages the background PDF loader has finished extracting
      // before we render. This keeps the page table in sync and triggers a
      // redraw if anything new arrived.
      if self.pdf_streaming.is_some() {
        let _ = self.drain_pdf_stream();
        if self
          .pdf_streaming
          .as_ref()
          .is_some_and(|state| !state.fully_loaded || state.ocr_loading)
        {
          self.mark_dirty();
        }
      }

      // Advance TTS narration: pull word-boundary events from the worker,
      // move the reading cursor, and repaint so the spoken-word highlight
      // tracks and the viewport auto-scrolls to follow the voice.
      if self.speech.is_some() {
        self.drain_speech();
      }

      // Manage the "Loaded in X.Xs" indicator: tick through the 500 ms
      // hold so the message appears promptly, then expire after 3 s.
      {
        let load_age = self
          .pdf_load_finished
          .as_ref()
          .map(|(t, _, _)| t.elapsed().as_secs_f32());
        if let Some(age) = load_age {
          if age >= 3.0 {
            self.pdf_load_finished = None;
            self.mark_dirty();
          } else if age < 0.55 {
            self.mark_dirty();
          }
        }
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
        if std::io::stdout().is_terminal() {
          let mut buffer = Vec::new();
          self.show_idle_cursor_if_needed(&mut buffer)?;
          if !buffer.is_empty() {
            stdout.write_all(&buffer)?;
            stdout.flush()?;
          }
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
          .map(|s| !s.fully_loaded || s.ocr_loading)
          .unwrap_or(false);
        let pending_pdf = self.pdf_pending.is_some();
        let load_transitioning = self
          .pdf_load_finished
          .as_ref()
          .map(|(t, _, _)| t.elapsed().as_secs_f32() < 0.55)
          .unwrap_or(false);
        let mut timeout = event_poll_timeout(
          self.needs_redraw,
          self.tutorial_demo_mode,
          streaming_active,
          pending_pdf,
          load_transitioning,
        );
        // While narrating, poll fast so the spoken-word highlight stays in
        // sync with the reading clock between word boundaries.
        if self.is_narrating() {
          timeout = std::time::Duration::from_millis(FAST_EVENT_POLL_MS);
        }

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

              // Any key press stops narration (press again to act normally).
              if self.is_narrating() {
                self.stop_narration();
                self.mark_dirty();
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

fn event_poll_timeout(
  needs_redraw: bool,
  tutorial_demo_mode: bool,
  streaming_active: bool,
  pending_pdf: bool,
  load_transitioning: bool,
) -> std::time::Duration {
  if needs_redraw || tutorial_demo_mode {
    std::time::Duration::from_millis(FAST_EVENT_POLL_MS)
  } else if streaming_active || pending_pdf || load_transitioning {
    std::time::Duration::from_millis(PDF_LOADING_EVENT_POLL_MS)
  } else {
    std::time::Duration::from_millis(IDLE_EVENT_POLL_MS)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::editor::streaming::{PendingPdfStream, StreamReady};
  use std::sync::mpsc;
  use std::time::Instant;

  fn test_editor() -> Editor {
    let mut editor = Editor::new(vec!["line".to_string()], 80);
    editor.height = 24;
    editor.width = 80;
    editor
  }

  fn rendered(buffer: Vec<u8>) -> String {
    String::from_utf8(buffer).expect("cursor commands should be utf8")
  }

  #[test]
  fn idle_cursor_show_marks_cursor_visible() {
    let mut editor = test_editor();
    editor.show_cursor = true;
    editor.cursor_currently_visible = false;

    let mut buffer = Vec::new();
    editor.show_idle_cursor_if_needed(&mut buffer).unwrap();

    assert!(rendered(buffer).contains("\x1b[?25h"));
    assert!(editor.cursor_currently_visible);
  }

  #[test]
  fn idle_cursor_show_skips_redundant_show_when_already_visible() {
    let mut editor = test_editor();
    editor.show_cursor = true;
    editor.cursor_currently_visible = true;

    let mut buffer = Vec::new();
    editor.show_idle_cursor_if_needed(&mut buffer).unwrap();

    assert!(buffer.is_empty());
    assert!(editor.cursor_currently_visible);
  }

  #[test]
  fn idle_cursor_show_skips_show_while_pdf_is_pending() {
    let mut editor = test_editor();
    editor.show_cursor = true;
    editor.cursor_currently_visible = false;
    let (_tx, rx) = mpsc::channel::<StreamReady>();
    editor.pdf_pending = Some(PendingPdfStream {
      receiver: rx,
      started_at: Instant::now(),
      canonical_path_display: "pending.pdf".to_string(),
      restore_line_in_page: None,
      restore_cursor_y: None,
    });

    let mut buffer = Vec::new();
    editor.show_idle_cursor_if_needed(&mut buffer).unwrap();

    assert!(buffer.is_empty());
    assert!(!editor.cursor_currently_visible);
  }

  #[test]
  fn event_poll_timeout_uses_spinner_cadence_for_pdf_loading() {
    assert_eq!(
      event_poll_timeout(false, false, true, false, false),
      std::time::Duration::from_millis(PDF_LOADING_EVENT_POLL_MS)
    );
    assert_eq!(
      event_poll_timeout(false, false, false, true, false),
      std::time::Duration::from_millis(PDF_LOADING_EVENT_POLL_MS)
    );
  }

  #[test]
  fn event_poll_timeout_keeps_fast_cadence_for_redraws_and_demo() {
    assert_eq!(
      event_poll_timeout(true, false, true, false, false),
      std::time::Duration::from_millis(FAST_EVENT_POLL_MS)
    );
    assert_eq!(
      event_poll_timeout(false, true, false, false, false),
      std::time::Duration::from_millis(FAST_EVENT_POLL_MS)
    );
  }
}
