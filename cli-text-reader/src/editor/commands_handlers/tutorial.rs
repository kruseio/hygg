use super::super::core::{Editor, EditorMode};

impl Editor {
  // Handle :help command - show help overlay
  pub fn handle_help_command(
    &mut self,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    let help_lines = crate::help::get_help_text();
    self.create_overlay("help", help_lines);
    self.set_active_mode(EditorMode::Normal);
    self.editor_state.command_buffer.clear();
    self.editor_state.command_cursor_pos = 0;
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.command_buffer.clear();
      buffer.command_cursor_pos = 0;
    }
    Ok(false)
  }

  // Handle :tutorial command - show interactive tutorial
  pub fn handle_tutorial_command(
    &mut self,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    self.show_interactive_tutorial()?;
    self.editor_state.command_buffer.clear();
    self.editor_state.command_cursor_pos = 0;
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.command_buffer.clear();
      buffer.command_cursor_pos = 0;
    }
    Ok(false)
  }

  // Handle :tutorial N command - jump to specific tutorial step
  pub fn handle_tutorial_command_with_step(
    &mut self,
    step: usize,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    self.debug_log(&format!("Starting tutorial at step {step}"));

    // Get the number of tutorial steps
    let steps = crate::interactive_tutorial::get_interactive_tutorial_steps();
    let max_step = steps.len();

    // Clamp the step to valid range (0-indexed internally, but 1-indexed for
    // users)
    let target_step = if step == 0 {
      0 // Allow :tutorial 0 to go to the first step
    } else if step > max_step {
      max_step - 1 // Go to last step if requested step is too high
    } else {
      step - 1 // Convert from 1-indexed to 0-indexed
    };

    // Start the tutorial
    self.show_interactive_tutorial()?;

    // Jump to the specified step
    self.tutorial_step = target_step;
    self.tutorial_step_completed = false;
    self.update_tutorial_step_internal(true);

    self.debug_log(&format!(
      "Jumped to tutorial step {step} (internal: {target_step})"
    ));

    self.editor_state.command_buffer.clear();
    self.editor_state.command_cursor_pos = 0;
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.command_buffer.clear();
      buffer.command_cursor_pos = 0;
    }
    Ok(false)
  }

  // Handle :next/:continue command for tutorial
  pub fn handle_next_command(
    &mut self,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    if self.tutorial_active {
      self.debug_log(&format!(
        "handle_next_command: tutorial_step={}, buffers={}, step_completed={}",
        self.tutorial_step,
        self.buffers.len(),
        self.tutorial_step_completed
      ));

      // Enhanced buffer validation with detailed logging
      if self.buffers.is_empty() {
        self.debug_log(
          "ERROR: No buffers available during tutorial next command",
        );
        self.complete_tutorial_interactive();
        return Ok(false);
      }

      // Validate we have at least 2 buffers (main + overlay) for tutorial
      if self.buffers.len() < 2 {
        self.debug_log(&format!(
          "WARNING: Only {} buffers, expected at least 2 for tutorial",
          self.buffers.len()
        ));
      }

      // Log buffer states for debugging
      for (i, buffer) in self.buffers.iter().enumerate() {
        self.debug_log(&format!(
          "  Buffer {}: lines={}, command={:?}, overlay_level={}",
          i,
          buffer.lines.len(),
          buffer.command,
          buffer.overlay_level
        ));
      }

      let steps = crate::interactive_tutorial::get_interactive_tutorial_steps();

      // Validate tutorial step is within bounds
      if self.tutorial_step >= steps.len() {
        self.debug_log(&format!(
          "ERROR: Tutorial step {} out of bounds (max: {})",
          self.tutorial_step,
          steps.len() - 1
        ));
        self.complete_tutorial_interactive();
        return Ok(false);
      }

      // Special handling for specific steps with enhanced logging
      let is_welcome = self.tutorial_step == 0;
      let is_congratulations = self.tutorial_step == steps.len() - 2; // Step before credits
      let is_credits = self.tutorial_step == steps.len() - 1;

      self.debug_log(&format!("  Step type: welcome={is_welcome}, congratulations={is_congratulations}, credits={is_credits}"));

      // Special handling for step 3 (Text Objects - Paragraph Selection)
      if self.tutorial_step == 3 {
        self
          .debug_log("  Special handling for step 3 - ensuring state is saved");
        // Force save state before advancing from step 3
        self.save_current_buffer_state();
      }

      if is_credits {
        // From credits screen, complete tutorial
        self.debug_log("  Completing tutorial from credits screen");
        self.complete_tutorial_interactive();
      } else if is_welcome || is_congratulations || self.tutorial_step_completed
      {
        // Allow advancement from welcome, congratulations (always), or any
        // completed step
        self.debug_log(&format!(
          "  Advancing tutorial (welcome: {}, congrats: {}, completed: {})",
          is_welcome, is_congratulations, self.tutorial_step_completed
        ));

        // Extra validation before advancing
        if self.active_buffer >= self.buffers.len() {
          self.debug_log(&format!(
            "WARNING: Active buffer {} out of range, resetting",
            self.active_buffer
          ));
          self.active_buffer = self.buffers.len().saturating_sub(1);
        }

        self.advance_tutorial();
      } else {
        // Step not completed yet
        self.debug_log("  Tutorial step not completed yet, cannot advance");
      }
    } else {
      self.debug_log("  Not in tutorial mode, ignoring :next command");
    }

    // Clear command state consistently
    self.set_active_mode(EditorMode::Normal);
    self.editor_state.command_buffer.clear();
    self.editor_state.command_cursor_pos = 0;

    // Clear command buffer in active buffer if it exists
    if self.active_buffer < self.buffers.len()
      && let Some(buffer) = self.buffers.get_mut(self.active_buffer)
    {
      buffer.command_buffer.clear();
      buffer.command_cursor_pos = 0;
    }

    Ok(false)
  }

  // Handle :back/:prev command for tutorial
  pub fn handle_back_command(
    &mut self,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    if self.tutorial_active {
      self.back_tutorial();
    }
    self.set_active_mode(EditorMode::Normal);
    self.editor_state.command_buffer.clear();
    self.editor_state.command_cursor_pos = 0;
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.command_buffer.clear();
      buffer.command_cursor_pos = 0;
    }
    Ok(false)
  }
}
