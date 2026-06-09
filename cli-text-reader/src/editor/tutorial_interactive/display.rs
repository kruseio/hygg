use super::super::core::Editor;
use crate::interactive_tutorial::{
  create_tutorial_buffer, get_interactive_tutorial_steps,
};

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
}
