use crossterm::event::{self, Event as CEvent};
use std::io;

use super::super::core::{Editor, EditorMode};
use super::{FAST_EVENT_POLL_MS, LoopControl, event_poll_timeout};

impl Editor {
  /// Handle one iteration of keyboard input while attached to a terminal:
  /// compute the poll cadence, run demo-script bookkeeping, then poll and
  /// dispatch a single crossterm event.
  pub(crate) fn handle_input_terminal(
    &mut self,
    stdout: &mut io::Stdout,
  ) -> Result<LoopControl, Box<dyn std::error::Error>> {
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
          return Ok(LoopControl::Break);
        }
        // handle_event will mark dirty if needed
        return Ok(LoopControl::Continue);
      }

      // Check immediately after demo progress - demo might have just
      // completed
      if self.should_exit_after_demo() {
        self.debug_log("Demo complete, exiting (immediate)");
        return Ok(LoopControl::Break);
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
      return Ok(LoopControl::Break);
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
            return Ok(LoopControl::Continue);
          }

          // Any key press stops narration (press again to act normally).
          if self.is_narrating() {
            self.stop_narration();
            self.mark_dirty();
            return Ok(LoopControl::Continue);
          }
          // A finished/failed narration leaves residual state (e.g. an
          // error message in the status line); the next key press dismisses
          // it, then is handled normally.
          if self.speech.is_some() {
            self.speech = None;
            self.mark_dirty();
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
              EditorMode::Command | EditorMode::CommandExecution => "Command",
              EditorMode::Tutorial => "Tutorial",
            }
          ));
          let exit = self.handle_event(key_event, stdout)?;

          if exit {
            self.debug_log("Exiting main loop");
            return Ok(LoopControl::Break);
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
      return Ok(LoopControl::Continue);
    }

    Ok(LoopControl::Proceed)
  }

  /// Handle one iteration when not attached to a terminal: only demo mode
  /// keeps spinning here, injecting scripted key events and sleeping.
  pub(crate) fn handle_input_non_terminal(
    &mut self,
    stdout: &mut io::Stdout,
  ) -> Result<LoopControl, Box<dyn std::error::Error>> {
    // In demo mode, continue even if not a terminal
    if !self.tutorial_demo_mode {
      self.debug_log("Not a terminal - exiting main loop");
      return Ok(LoopControl::Break);
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
        return Ok(LoopControl::Break);
      }
      // handle_event will mark dirty if needed
    }

    // Check if demo should exit
    if self.should_exit_after_demo() {
      self.debug_log("Demo complete, exiting (non-terminal)");
      return Ok(LoopControl::Break);
    }

    // Wait a bit and continue
    std::thread::sleep(std::time::Duration::from_millis(50));

    Ok(LoopControl::Proceed)
  }
}
