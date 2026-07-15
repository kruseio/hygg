use super::core::{Editor, ViewMode};

impl Editor {
  // Helper method to move to a specific position
  pub fn move_to_position(&mut self, target_line: usize, target_col: usize) {
    if target_line != self.offset + self.cursor_y {
      // Line changed - use existing navigation logic
      self.goto_line_with_overscroll(target_line);
    }
    self.cursor_x = target_col;
    self.cursor_moved = true;

    // No longer need to save state after every movement since we use live state
    // for active buffer

    self.mark_dirty();
  }

  // Move cursor down one line, handling boundary conditions
  pub fn move_cursor_down(&mut self) {
    // Get the current absolute line position
    let current_line = self.offset + self.cursor_y;

    self.debug_log(&format!(
      "move_cursor_down: current_line={}, total_lines={}, offset={}, cursor_y={}, active_buffer={}",
      current_line, self.total_lines, self.offset, self.cursor_y, self.active_buffer
    ));

    // Strict boundary check - cannot move beyond last line
    if current_line >= self.total_lines.saturating_sub(1) {
      self.debug_log("  Cannot move down - at last line");
      return; // Already at last line, cannot move down
    }

    self.cursor_moved = true;

    // Use effective viewport height
    let viewport_height = self.get_effective_viewport_height();
    self.debug_log(&format!("  viewport_height={viewport_height}"));

    // Check if we can move within the current viewport
    if self.cursor_y < viewport_height.saturating_sub(1) {
      // Try to move cursor down within current viewport
      self.cursor_y += 1;
      self.debug_log(&format!(
        "  Moving within viewport: cursor_y={}",
        self.cursor_y
      ));
    } else {
      // At bottom of viewport, need to scroll
      self.offset += 1;
      self.debug_log(&format!("  Scrolling down: offset={}", self.offset));
    }

    // Ensure cursor_y stays within viewport bounds
    let max_cursor_y = viewport_height.saturating_sub(1);
    if self.cursor_y > max_cursor_y {
      self.debug_log(&format!(
        "move_cursor_down: Adjusting cursor_y {} to max {}",
        self.cursor_y, max_cursor_y
      ));
      self.cursor_y = max_cursor_y;
    }

    // Keep cursor position on the current line
    let current_line_idx = self.offset + self.cursor_y;
    if let Some(line) = self.lines.get(current_line_idx) {
      self.cursor_x = self.cursor_x.min(line.len().saturating_sub(1));
    }

    // No longer need to save state after every movement since we use live state
    // for active buffer

    self.mark_dirty();
  }

  // Move cursor up one line, handling boundary conditions
  pub fn move_cursor_up(&mut self) {
    // Get the current absolute line position
    let current_line = self.offset + self.cursor_y;

    // Strict boundary check - cannot move beyond first line
    if current_line == 0 {
      return; // Already at first line, cannot move up
    }

    self.cursor_moved = true;

    // Use effective viewport height
    let viewport_height = self.get_effective_viewport_height();

    // Check if we can move within the current viewport
    if self.cursor_y > 0 {
      // Try to move cursor up within current viewport
      self.cursor_y -= 1;
      self.debug_log(&format!(
        "  Moving within viewport: cursor_y={}",
        self.cursor_y
      ));
    } else if self.offset > 0 {
      // At top of viewport, need to scroll
      self.offset -= 1;
      self.debug_log(&format!("  Scrolling up: offset={}", self.offset));
    }

    // Ensure cursor_y stays within viewport bounds
    let max_cursor_y = viewport_height.saturating_sub(1);
    if self.cursor_y > max_cursor_y {
      self.debug_log(&format!(
        "move_cursor_up: Adjusting cursor_y {} to max {}",
        self.cursor_y, max_cursor_y
      ));
      self.cursor_y = max_cursor_y;
    }

    // Keep cursor position on the current line
    let current_line_idx = self.offset + self.cursor_y;
    if let Some(line) = self.lines.get(current_line_idx) {
      self.cursor_x = self.cursor_x.min(line.len().saturating_sub(1));
    }

    // No longer need to save state after every movement since we use live state
    // for active buffer

    self.mark_dirty();
  }

  // Move cursor left one character
  pub fn move_cursor_left(&mut self) {
    if self.cursor_x > 0 {
      self.cursor_x -= 1;
      self.cursor_moved = true;

      // Save state back to buffer if in split view
      if self.view_mode == ViewMode::HorizontalSplit {
        self.save_current_buffer_state();
      }

      self.mark_dirty();
    }
  }

  // Move cursor right one character
  pub fn move_cursor_right(&mut self) {
    let current_line_idx = self.offset + self.cursor_y;
    if current_line_idx < self.lines.len() {
      let line_length = self.lines[current_line_idx].len();
      if self.cursor_x < line_length.saturating_sub(1) {
        self.cursor_x += 1;
        self.cursor_moved = true;

        // Save state back to buffer if in split view
        if self.view_mode == ViewMode::HorizontalSplit {
          self.save_current_buffer_state();
        }

        self.mark_dirty();
      }
    }
  }

  // Move cursor to word boundary
  pub fn move_to_word_boundary(&mut self, forward: bool, big_word: bool) {
    let (new_line, new_col) = self.find_word_boundary(forward, big_word);

    if new_line != self.offset + self.cursor_y {
      // Line changed - need to update cursor position properly
      let viewport_height = match &self.view_mode {
        ViewMode::Normal => self.height.saturating_sub(1),
        ViewMode::Overlay => self.height.saturating_sub(1),
        ViewMode::HorizontalSplit => {
          // In split mode, use the height of the active pane
          if self.active_pane == 0 {
            (self.height.saturating_sub(1) as f32 * self.split_ratio) as usize
          } else {
            self
              .height
              .saturating_sub(1)
              .saturating_sub(
                (self.height.saturating_sub(1) as f32 * self.split_ratio)
                  as usize,
              )
              .saturating_sub(1)
          }
        }
      };
      let center_y = viewport_height / 2;

      if new_line >= self.offset && new_line < self.offset + viewport_height {
        // New line is visible on screen
        self.cursor_y = new_line - self.offset;
      } else {
        // New line is off-screen, center it with overscroll capability
        if new_line < center_y {
          // Near beginning of document
          self.offset = 0;
          self.cursor_y = new_line;
        } else {
          // Use overscroll-style centering
          self.offset = new_line.saturating_sub(center_y);
          self.cursor_y = center_y;
        }
      }
    }

    self.cursor_x = new_col;

    // Use appropriate centering based on view mode
    match &self.view_mode {
      ViewMode::Normal => self.center_cursor_with_overscroll(true),
      ViewMode::Overlay => self.center_cursor_with_overscroll(true),
      ViewMode::HorizontalSplit => self.center_cursor_with_overscroll(true),
    }

    // No longer need to save state after every movement since we use live state
    // for active buffer

    self.mark_dirty();
  }
}
