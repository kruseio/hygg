use super::super::core::{Editor, EditorMode};
use crate::config::{AppConfig, save_config};
use crate::interactive_tutorial::get_interactive_tutorial_steps;
use crate::interactive_tutorial_buffer::TutorialSuccessCondition;
use crossterm::event::KeyCode;

impl Editor {
  // Handle tutorial progression
  pub fn advance_tutorial(&mut self) {
    let steps = get_interactive_tutorial_steps();
    self.debug_log(&format!(
      "Advancing tutorial from step {} (total steps: {})",
      self.tutorial_step,
      steps.len()
    ));

    // Safety check to prevent out of bounds
    if self.tutorial_step >= steps.len() {
      self.debug_log("Tutorial step out of bounds, completing tutorial");
      self.complete_tutorial_interactive();
      return;
    }

    if self.tutorial_step < steps.len() - 1 {
      // Critical: Save buffer state BEFORE any modifications
      self.debug_log(&format!(
        "Saving buffer state before advancing (buffers: {}, active: {})",
        self.buffers.len(),
        self.active_buffer
      ));

      // Validate that we have buffers before proceeding
      if self.buffers.is_empty() {
        self.debug_log("ERROR: No buffers available for tutorial advance");
        self.complete_tutorial_interactive();
        return;
      }

      // Ensure active buffer index is valid
      if self.active_buffer >= self.buffers.len() {
        self.debug_log(&format!(
          "WARNING: Active buffer {} out of range, resetting to 0",
          self.active_buffer
        ));
        self.active_buffer = 0;
      }

      // Save current buffer state before any modifications
      self.save_current_buffer_state();

      // Clear highlights when advancing FROM the highlighting step (step 3)
      // This ensures highlights don't carry over to subsequent steps
      if self.tutorial_step == 3 {
        self.debug_log("Clearing highlights before advancing from step 3");
        self.highlights.clear_all_highlights();
      }

      self.tutorial_step += 1;
      self.debug_log(&format!(
        "Advanced to tutorial step {}",
        self.tutorial_step
      ));

      // Reset completion flag for the new step
      self.tutorial_step_completed = false;

      // Final validation before updating
      if self.buffers.is_empty() {
        self.debug_log("ERROR: Buffers became empty during advance");
        self.complete_tutorial_interactive();
        return;
      }

      // Debug log buffer state before update
      self.debug_log(&format!("Before update_tutorial_step_internal: buffers={}, active={}, lines in active={}",
        self.buffers.len(),
        self.active_buffer,
        self.buffers.get(self.active_buffer).map(|b| b.lines.len()).unwrap_or(0)
      ));

      // Update with is_new_step=true to clear state from previous step
      self.update_tutorial_step_internal(true);
    } else {
      self.complete_tutorial_interactive();
    }
  }

  // Go back to previous tutorial step
  pub fn back_tutorial(&mut self) {
    self.debug_log(&format!(
      "Going back from tutorial step {}",
      self.tutorial_step
    ));

    if self.tutorial_step > 0 {
      self.tutorial_step -= 1;

      // Reset completion flag for the step we're going back to
      self.tutorial_step_completed = false;
      // Clear any highlights when going back
      self.highlights.clear_all_highlights();
      // Update with is_new_step=true to reset state
      self.update_tutorial_step_internal(true);
    }
  }

  // Complete the tutorial
  pub fn complete_tutorial_interactive(&mut self) {
    self.debug_log("Completing interactive tutorial");
    self.debug_log(&format!(
      "Current buffers: {}, active: {}",
      self.buffers.len(),
      self.active_buffer
    ));

    // Reset flags
    self.tutorial_active = false;
    self.current_tutorial_condition = None;
    self.tutorial_step_completed = false;

    // Save config
    let config = AppConfig {
      enable_tutorial: None,
      enable_line_highlighter: None,
      show_cursor: None,
      show_progress: None,
      pdf_ocr: None,
      tutorial_shown: Some(true),
    };

    if let Err(e) = save_config(&config) {
      self.debug_log_error(&format!("Failed to save tutorial state: {e}"));
    }

    // Close overlay and return to normal mode with original document
    self.debug_log("Closing tutorial overlay, returning to original document");
    self.close_overlay();
    self.set_active_mode(EditorMode::Normal);

    // Restore cursor position
    if let Some((line, col)) = self.previous_position {
      self.debug_log(&format!(
        "Restoring cursor position to line {line}, col {col}"
      ));
      if line < self.lines.len() {
        self.offset = line.saturating_sub(self.height / 2);
        self.cursor_y = line.saturating_sub(self.offset).min(self.height - 2);
        self.cursor_x = col;
      }
    }

    self.debug_log(&format!(
      "After close: buffers: {}, active: {}",
      self.buffers.len(),
      self.active_buffer
    ));
  }

  // Process keys during tutorial - let normal editor handle most things
  pub fn process_tutorial_key(&mut self, key: KeyCode) -> bool {
    self.debug_log(&format!(
      "Tutorial processing key: {:?}, step_completed: {}",
      key, self.tutorial_step_completed
    ));

    // Tutorial can only be exited with :q command, not just 'q'
    // This ensures users learn the proper command mode

    // If step is completed, allow all normal movement but show the :next hint
    if self.tutorial_step_completed {
      // Just allow normal key processing - don't restrict movement
      return false;
    }

    // Check for specific key presses if that's what we're waiting for
    if let Some(TutorialSuccessCondition::KeyPress(expected)) =
      &self.current_tutorial_condition
      && !self.tutorial_step_completed
      && match key {
        KeyCode::Char(c) => c.to_string() == *expected,
        KeyCode::Down => expected == "j" || expected == "Down",
        KeyCode::Up => expected == "k" || expected == "Up",
        _ => false,
      }
    {
      // Mark step as completed but don't advance
      self.tutorial_step_completed = true;
      // Update the display to show the ":next" hint
      self.update_tutorial_step();
      // Don't return early - let the key be processed for movement
    }

    // For final step with NoCondition, allow :next to return to document
    if let Some(TutorialSuccessCondition::NoCondition) =
      &self.current_tutorial_condition
    {
      // Don't handle Enter here, let them use :next command
      return false;
    }

    // After any other action, check if the success condition is met
    if !self.tutorial_step_completed && self.check_tutorial_completion() {
      // Mark as completed and update display to show ":next" hint
      self.tutorial_step_completed = true;
      self.update_tutorial_step();
    }

    // Always return false to allow normal key processing for movement
    false
  }
}
