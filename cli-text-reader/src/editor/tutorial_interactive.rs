use super::core::{Editor, EditorMode};
use crate::config::{AppConfig, save_config};
use crate::interactive_tutorial::{
  create_tutorial_buffer, get_interactive_tutorial_steps,
};
use crate::interactive_tutorial_buffer::TutorialSuccessCondition;
use crossterm::event::KeyCode;

impl Editor {
  // Show the interactive tutorial
  pub fn show_interactive_tutorial(
    &mut self,
  ) -> Result<(), Box<dyn std::error::Error>> {
    self.debug_log("Starting simple interactive tutorial");

    // Save current cursor position
    self.previous_position = Some((self.offset + self.cursor_y, self.cursor_x));

    // Clear all highlights for the tutorial
    self.highlights.clear_all_highlights();

    // Set tutorial mode
    self.tutorial_active = true;
    self.tutorial_step = 0;

    // Show the first step (with is_new_step=true to clear any existing state)
    self.update_tutorial_step_internal(true);

    Ok(())
  }

  // Update tutorial display for current step
  pub fn update_tutorial_step(&mut self) {
    self.update_tutorial_step_internal(false);
  }

  // Internal method that can optionally preserve state
  pub fn update_tutorial_step_internal(&mut self, is_new_step: bool) {
    self.debug_log(&format!(
      "update_tutorial_step_internal called: is_new_step={}, step={}",
      is_new_step, self.tutorial_step
    ));

    let steps = get_interactive_tutorial_steps();
    if self.tutorial_step >= steps.len() {
      self.debug_log(
        "Tutorial step out of range in update_tutorial_step_internal",
      );
      self.complete_tutorial_interactive();
      return;
    }

    // Enhanced terminal dimension validation
    if self.height < 5 || self.width < 20 {
      self.debug_log(&format!(
        "ERROR: Terminal too small for tutorial: {}x{} (min: 20x5)",
        self.width, self.height
      ));
      // Add a minimal placeholder content instead of returning
      let minimal_lines =
        vec!["Terminal too small".to_string(), "Please resize".to_string()];
      self.create_overlay("tutorial", minimal_lines);
      return;
    }

    // Validate buffer state before proceeding
    if self.buffers.is_empty() {
      self.debug_log(
        "ERROR: No buffers available in update_tutorial_step_internal",
      );
      return;
    }

    let step = &steps[self.tutorial_step];
    // Leave space for command line and potential errors
    let available_height = self.height.saturating_sub(2);

    self.debug_log(&format!(
      "Creating tutorial buffer for step {} with width {}",
      self.tutorial_step, self.width
    ));

    let buffer_lines = create_tutorial_buffer(
      step,
      self.tutorial_step,
      steps.len(),
      self.width,
      self.tutorial_step_completed,
    );

    // Ensure buffer doesn't exceed available height
    // Reserve extra space to ensure command line is always visible
    let safe_height = available_height.saturating_sub(3).max(1);
    let mut truncated_lines = buffer_lines;

    // Enhanced empty lines protection
    if truncated_lines.is_empty() {
      self.debug_log(
        "WARNING: Tutorial buffer has no lines, adding default content",
      );
      truncated_lines.push(format!(
        "Tutorial Step {} of {}",
        self.tutorial_step + 1,
        steps.len()
      ));
      truncated_lines.push("Content loading...".to_string());
      truncated_lines.push("".to_string());
      truncated_lines.push("Press :q to exit tutorial".to_string());
    }

    // Apply height truncation after ensuring non-empty
    if truncated_lines.len() > safe_height && safe_height > 0 {
      self.debug_log(&format!(
        "Truncating tutorial lines from {} to {}",
        truncated_lines.len(),
        safe_height
      ));
      truncated_lines.truncate(safe_height);
    }

    self.debug_log(&format!(
      "Creating tutorial overlay with {} lines (safe_height: {})",
      truncated_lines.len(),
      safe_height
    ));

    // Log buffer state before overlay creation
    self.debug_log(&format!(
      "Pre-overlay state: buffers={}, active={}",
      self.buffers.len(),
      self.active_buffer
    ));

    // Create overlay with the tutorial content
    self.create_overlay("tutorial", truncated_lines);

    // Log buffer state after overlay creation
    self.debug_log(&format!(
      "Post-overlay state: buffers={}, active={}",
      self.buffers.len(),
      self.active_buffer
    ));

    // Store current step's success condition
    self.current_tutorial_condition = Some(step.success_check.clone());

    // Only reset state if we're advancing to a new step
    if is_new_step {
      self.debug_log("Resetting tutorial state for new step");

      // Reset success flags to prevent bleed-through from previous steps
      self.tutorial_highlight_created = false;
      self.tutorial_yank_performed = false;
      self.tutorial_paste_performed = false;
      self.tutorial_search_navigated = false;
      self.tutorial_bookmark_jumped = false;
      self.tutorial_forward_search_used = false;
      self.tutorial_backward_search_used = false;
      self.last_executed_command = None;

      // Clear search highlights to prevent bleed-through
      self.editor_state.search_query.clear();
      self.editor_state.current_match = None;

      // Sync search state with active buffer - with bounds checking
      if self.active_buffer < self.buffers.len() {
        if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
          buffer.search_query.clear();
          buffer.current_match = None;
        }
      } else {
        self.debug_log(&format!(
          "WARNING: Active buffer {} out of range in state reset",
          self.active_buffer
        ));
      }
    }

    self.mark_dirty();
  }

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
