use super::core::{BufferState, Editor, EditorMode, ViewMode};
use cli_pdf_to_text::PdfLineKind;

impl Editor {
  // Create or update the single overlay buffer
  pub fn create_overlay(&mut self, cmd: &str, lines: Vec<String>) {
    self.debug_log("=== create_overlay ===");
    self.debug_log(&format!("  Command: '{}', Lines: {}", cmd, lines.len()));
    self.debug_log(&format!(
      "  Current state: buffers={}, active={}, view_mode={:?}",
      self.buffers.len(),
      self.active_buffer,
      self.view_mode
    ));

    // Enhanced input validation with string content checking
    let validated_lines = if lines.is_empty() {
      self.debug_log(
        "  WARNING: Empty lines provided to create_overlay, using placeholder",
      );
      vec!["No content available".to_string(), "Press :q to exit".to_string()]
    } else {
      // Validate each line to ensure it's properly formed
      let mut validated = Vec::with_capacity(lines.len());
      for (i, line) in lines.into_iter().enumerate() {
        // Check if the string is valid UTF-8 and handle any issues
        match line.chars().count() {
          0 => {
            // Empty line is OK
            validated.push(line);
          }
          _ => {
            // Non-empty line - ensure it's valid
            if line.as_bytes().iter().all(|&b| b != 0) {
              validated.push(line);
            } else {
              self.debug_log(&format!(
                "  WARNING: Line {i} contains null bytes, replacing"
              ));
              validated.push("INVALID_LINE".to_string());
            }
          }
        }
      }
      if validated.is_empty() {
        self.debug_log("  WARNING: All lines were invalid, using placeholder");
        vec!["No content available".to_string(), "Press :q to exit".to_string()]
      } else {
        validated
      }
    };

    // Save current buffer state before ANY modifications
    if self.active_buffer < self.buffers.len() {
      self.debug_log(&format!(
        "  Saving state for buffer {} before overlay creation",
        self.active_buffer
      ));
      self.save_current_buffer_state();
    } else {
      self.debug_log(&format!(
        "  WARNING: Active buffer {} out of range, cannot save state",
        self.active_buffer
      ));
    }

    // Ensure we have a main buffer (index 0)
    if self.buffers.is_empty() {
      self.debug_log("  ERROR: No main buffer exists, creating minimal buffer");
      // Create a minimal main buffer to prevent crash
      let minimal_buffer = BufferState::new(vec!["".to_string()]);
      self.buffers.push(minimal_buffer);
    }

    // Validate main buffer exists and has content
    if let Some(main_buffer) = self.buffers.first()
      && main_buffer.lines.is_empty()
    {
      self.debug_log("  WARNING: Main buffer has no lines");
    }

    // Check if we have an overlay buffer (index 1)
    if self.buffers.len() > 1 {
      // Update existing overlay buffer
      self.debug_log("  Updating existing overlay buffer");

      // Log details about the lines we're about to assign
      self.debug_log(&format!(
        "  About to update overlay buffer with {} lines",
        validated_lines.len()
      ));
      if !validated_lines.is_empty() {
        // Take 30 *characters*, not 30 bytes: overlay lines carry Unicode box
        // and progress glyphs (━ █ ░) whose UTF-8 runs the byte slice would cut
        // mid-character, panicking a debug preview that has nothing to do with
        // the render. This fires on every overlay update — tutorial steps,
        // help, credits — so it cannot be left to chance.
        let preview = |l: &String| l.chars().take(30).collect::<String>();
        self.debug_log(&format!(
          "  First line preview: {:?}",
          validated_lines.first().map(preview)
        ));
        self.debug_log(&format!(
          "  Last line preview: {:?}",
          validated_lines.last().map(preview)
        ));
      }

      if let Some(overlay_buffer) = self.buffers.get_mut(1) {
        let prev_lines_count = overlay_buffer.lines.len();

        overlay_buffer.lines = validated_lines;
        overlay_buffer.line_kinds =
          vec![PdfLineKind::Text; overlay_buffer.lines.len()];
        overlay_buffer.command = Some(cmd.to_string());
        overlay_buffer.offset = 0;

        // Enhanced cursor bounds validation
        let max_y = overlay_buffer.lines.len().saturating_sub(1);
        let old_cursor_y = overlay_buffer.cursor_y;
        if overlay_buffer.cursor_y > max_y {
          overlay_buffer.cursor_y = max_y;
        }

        // Validate cursor_x within line bounds
        let cursor_y = overlay_buffer.cursor_y;
        if let Some(line) = overlay_buffer.lines.get(cursor_y) {
          let max_x = line.len();
          let _old_cursor_x = overlay_buffer.cursor_x;
          if overlay_buffer.cursor_x > max_x {
            overlay_buffer.cursor_x = max_x;
          }
        }

        self.total_lines = overlay_buffer.lines.len();

        // Extract final cursor values for logging
        let _final_cursor_x = overlay_buffer.cursor_x;
        let final_cursor_y = overlay_buffer.cursor_y;
        self.debug_log(&format!(
          "  Updated overlay: prev_lines={}, new_lines={}",
          prev_lines_count, self.total_lines
        ));

        // Log cursor adjustments if any
        if old_cursor_y != final_cursor_y {
          self.debug_log(&format!(
            "  Adjusted cursor_y from {old_cursor_y} to {final_cursor_y}"
          ));
        }
      } else {
        self.debug_log("  ERROR: Cannot access overlay buffer at index 1");
        return;
      }
    } else {
      // Create new overlay buffer
      self.debug_log("  Creating new overlay buffer");
      let mut buffer = BufferState::new(validated_lines);
      buffer.command = Some(cmd.to_string());
      buffer.viewport_height = self.height.saturating_sub(1).max(1);
      buffer.viewport_start = 0;
      buffer.overlay_level = 1;

      // Ensure cursor is within bounds for new buffer
      buffer.cursor_y = 0;
      buffer.cursor_x = 0;

      self.buffers.push(buffer);
      self.debug_log(&format!(
        "  Created overlay buffer, total buffers now: {}",
        self.buffers.len()
      ));
    }

    // Final validation before switching
    if self.buffers.len() <= 1 {
      self.debug_log("  ERROR: Failed to create/update overlay buffer");
      return;
    }

    // Switch to overlay buffer
    self.debug_log(&format!(
      "  Switching from buffer {} to overlay buffer 1",
      self.active_buffer
    ));
    self.active_buffer = 1;
    self.view_mode = ViewMode::Overlay;

    // Load the overlay buffer's state with validation
    self.debug_log("  Loading overlay buffer state");
    self.load_buffer_state(1);

    // Reset to normal mode
    self.set_active_mode(EditorMode::Normal);
    self.editor_state.command_buffer.clear();
    self.editor_state.command_cursor_pos = 0;

    self.debug_log(&format!("  Now showing overlay with command: {cmd}"));
    self.debug_log(&format!(
      "  Final state: buffers={}, active={}, view_mode={:?}",
      self.buffers.len(),
      self.active_buffer,
      self.view_mode
    ));
    self.debug_log("=== create_overlay complete ===");
  }

  // Close overlay and return to main buffer
  pub fn close_overlay(&mut self) -> bool {
    self.debug_log("=== close_overlay ===");

    if self.view_mode != ViewMode::Overlay {
      self.debug_log("  Not in overlay mode, nothing to close");
      return false;
    }

    // Remove overlay buffer and switch back to main buffer
    if self.buffers.len() > 1 {
      self.buffers.pop(); // Remove overlay buffer
    }
    self.active_buffer = 0;
    self.view_mode = ViewMode::Normal;

    // Load main buffer state
    self.load_buffer_state(0);

    self.debug_log("  Returned to main buffer");
    self.debug_log("=== close_overlay complete ===");
    true
  }

  // Check if we can close the current buffer
  pub fn can_close_buffer(&self) -> bool {
    self.buffers.len() > 1
  }
}
