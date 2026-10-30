use super::char_boundary::{CursorCol, DisplayCol, Pos};
use super::core::{Editor, ViewMode};
use crate::core_types::DesiredColumn;

impl Editor {
  // The display column j/k should aim for: the remembered one if the cursor
  // is still where the last j/k (or `$`) left it, otherwise the cursor's own
  // on-screen column.
  fn desired_display_column(&self) -> DisplayCol {
    let line_idx = self.offset + self.cursor_y;
    if let Some(desired) = self.editor_state.desired_column
      && desired.line == line_idx
      && desired.cursor_x == self.cursor_x
    {
      return desired.column;
    }
    self
      .lines
      .get(line_idx)
      .map_or(DisplayCol::default(), |line| self.cursor_x.display_col(line))
  }

  // Put the cursor on the grapheme covering display column `column` of the
  // current line (clamped to the last grapheme) and remember the column for
  // the next vertical motion.
  fn land_on_display_column(&mut self, column: DisplayCol) {
    let line_idx = self.offset + self.cursor_y;
    if let Some(line) = self.lines.get(line_idx) {
      self.cursor_x = CursorCol::from_display_col(line, column);
    }
    self.remember_display_column(column);
  }

  // Remember `column` as where j/k should aim, anchored to the current cursor
  // position so any other motion invalidates it.
  pub(crate) fn remember_display_column(&mut self, column: DisplayCol) {
    self.editor_state.desired_column = Some(DesiredColumn {
      column,
      line: self.offset + self.cursor_y,
      cursor_x: self.cursor_x,
    });
  }

  // Helper method to move to a specific position
  pub fn move_to_position(&mut self, target: Pos) {
    if target.line != self.offset + self.cursor_y {
      // Line changed - use existing navigation logic
      self.goto_line_with_overscroll(target.line);
    }
    // Settle onto the start of a grapheme of the line the cursor landed on,
    // so it never sits inside a visible character.
    self.cursor_x = self
      .lines
      .get(self.offset + self.cursor_y)
      .map_or(CursorCol::ZERO, |line| CursorCol::floor(line, target.col));
    self.editor_state.desired_column = None;
    self.cursor_moved = true;

    // No longer need to save state after every movement since we use live state
    // for active buffer

    self.mark_dirty();
  }

  // Move cursor down one line, handling boundary conditions
  pub fn move_cursor_down(&mut self) {
    // Get the current absolute line position
    let current_line = self.offset + self.cursor_y;
    let desired_column = self.desired_display_column();

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

    // Keep the on-screen column: land on the grapheme under it, or the last
    // one on a shorter line, and remember it so the next j/k restores it.
    self.land_on_display_column(desired_column);

    // No longer need to save state after every movement since we use live state
    // for active buffer

    self.mark_dirty();
  }

  // Move cursor up one line, handling boundary conditions
  pub fn move_cursor_up(&mut self) {
    // Get the current absolute line position
    let current_line = self.offset + self.cursor_y;
    let desired_column = self.desired_display_column();

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

    // Keep the on-screen column: land on the grapheme under it, or the last
    // one on a shorter line, and remember it so the next j/k restores it.
    self.land_on_display_column(desired_column);

    // No longer need to save state after every movement since we use live state
    // for active buffer

    self.mark_dirty();
  }

  // Move cursor left one visible character: `cursor_x` is a byte offset, so
  // step back over the whole previous grapheme cluster, not one byte or one
  // char of it (issue #99).
  pub fn move_cursor_left(&mut self) {
    let current_line_idx = self.offset + self.cursor_y;
    if self.cursor_x > CursorCol::ZERO
      && let Some(line) = self.lines.get(current_line_idx)
    {
      self.cursor_x = self.cursor_x.prev(line);
      self.editor_state.desired_column = None;
      self.cursor_moved = true;

      // Save state back to buffer if in split view
      if self.view_mode == ViewMode::HorizontalSplit {
        self.save_current_buffer_state();
      }

      self.mark_dirty();
    }
  }

  // Move cursor right one visible character. Steps over the whole current
  // grapheme cluster and stops on the start of the last one (issue #99).
  pub fn move_cursor_right(&mut self) {
    let current_line_idx = self.offset + self.cursor_y;
    if current_line_idx < self.lines.len() {
      let line = &self.lines[current_line_idx];
      if self.cursor_x < CursorCol::last(line) {
        self.cursor_x = self.cursor_x.next(line);
        self.editor_state.desired_column = None;
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
    let target = self.find_word_boundary(forward, big_word);
    let new_line = target.line;

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

    // Settle onto a grapheme start of the target line (issue #99).
    self.cursor_x = self
      .lines
      .get(new_line)
      .map_or(CursorCol::ZERO, |line| CursorCol::floor(line, target.col));
    self.editor_state.desired_column = None;

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

#[cfg(test)]
mod tests;
