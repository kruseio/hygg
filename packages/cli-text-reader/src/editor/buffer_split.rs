use super::core::{BufferState, Editor, EditorMode, SplitPosition, ViewMode};

impl Editor {
  // Create horizontal split with command output
  pub fn create_horizontal_split(&mut self, cmd: &str, lines: Vec<String>) {
    self.debug_log("=== create_horizontal_split ===");
    self.debug_log(&format!("  Command: '{}', Lines: {}", cmd, lines.len()));
    self.debug_log(&format!(
      "  Tutorial mode: {}, buffers: {}",
      self.tutorial_active,
      self.buffers.len()
    ));

    // Save current buffer state before switching
    if self.active_buffer < self.buffers.len() {
      self.save_current_buffer_state();
    }

    // Debug: Check buffer 0 state
    if let Some(buf0) = self.buffers.first() {
      self.debug_log(&format!("  Buffer 0 state before split: lines={}, viewport_height={}, mode={:?}", 
        buf0.lines.len(), buf0.viewport_height, buf0.mode));
      if buf0.lines.is_empty() {
        self.debug_log("  WARNING: Buffer 0 has no lines!");
      }
    } else {
      self.debug_log("  WARNING: Buffer 0 does not exist!");
    }

    // Calculate split heights
    let terminal_height = self.height.saturating_sub(1); // Subtract status line
    let top_height = (terminal_height as f32 * self.split_ratio) as usize;
    let bottom_height = terminal_height.saturating_sub(top_height);

    self.debug_log(&format!(
      "  Terminal height: {terminal_height}, Top: {top_height}, Bottom: {bottom_height}"
    ));

    // Determine which buffer should be shown in the top pane
    let top_buffer_idx = if self.tutorial_active && self.buffers.len() > 1 {
      // In tutorial mode, show the tutorial overlay (buffer 1) in top pane
      1
    } else {
      // Normal mode, show main buffer (buffer 0) in top pane
      0
    };

    // Update the top buffer's viewport for split
    if let Some(top_buffer) = self.buffers.get_mut(top_buffer_idx) {
      top_buffer.viewport_height = top_height;
      top_buffer.split_height = Some(top_height);
      top_buffer.split_position = SplitPosition::Top;
    }

    // Determine where to place the command output buffer
    let cmd_buffer_idx = if self.tutorial_active && self.buffers.len() > 1 {
      // In tutorial mode, we need to create/update buffer at index 2
      2
    } else {
      // Normal mode, use buffer index 1
      1
    };

    // Check if we need to create or update the command buffer
    if self.buffers.len() > cmd_buffer_idx {
      // Update existing command buffer
      self.debug_log(&format!(
        "  Updating existing command buffer at index {cmd_buffer_idx}"
      ));
      self.buffers[cmd_buffer_idx].lines = lines;
      self.buffers[cmd_buffer_idx].command = Some(cmd.to_string());
      self.buffers[cmd_buffer_idx].offset = 0;
      self.buffers[cmd_buffer_idx].cursor_x = 0;
      self.buffers[cmd_buffer_idx].cursor_y = 0;
      self.buffers[cmd_buffer_idx].viewport_height = bottom_height;
      self.buffers[cmd_buffer_idx].split_height = Some(bottom_height);
      self.buffers[cmd_buffer_idx].is_split_buffer = true;
      self.buffers[cmd_buffer_idx].split_position = SplitPosition::Bottom;
      self.total_lines = self.buffers[cmd_buffer_idx].lines.len();
    } else {
      // Create new command buffer
      self.debug_log(&format!(
        "  Creating new command buffer at index {cmd_buffer_idx}"
      ));
      let mut buffer = BufferState::new(lines);
      buffer.command = Some(cmd.to_string());
      buffer.viewport_height = bottom_height;
      buffer.viewport_start = 0;
      buffer.overlay_level = if cmd_buffer_idx == 2 { 2 } else { 1 };
      buffer.split_height = Some(bottom_height);
      buffer.is_split_buffer = true;
      buffer.split_position = SplitPosition::Bottom;

      // Ensure we have the right number of buffers
      while self.buffers.len() <= cmd_buffer_idx {
        self.buffers.push(buffer.clone());
      }
      self.buffers[cmd_buffer_idx] = buffer;
    }

    // Switch to split view mode
    self.view_mode = ViewMode::HorizontalSplit;
    self.active_buffer = cmd_buffer_idx; // Focus on command output
    self.active_pane = 1; // Bottom pane

    // Load the command buffer's state
    self.load_buffer_state(cmd_buffer_idx);

    // Reset to normal mode
    self.set_active_mode(EditorMode::Normal);
    self.editor_state.command_buffer.clear();
    self.editor_state.command_cursor_pos = 0;

    self.debug_log(&format!(
      "  Now showing horizontal split with command: {cmd}"
    ));
    self.debug_log("=== create_horizontal_split complete ===");
  }

  // Close split and return to normal view
  pub fn close_split(&mut self) -> bool {
    self.debug_log("=== close_split ===");
    self.debug_log(&format!("  Buffers before close: {}", self.buffers.len()));

    if self.view_mode != ViewMode::HorizontalSplit {
      self.debug_log("  Not in split mode, nothing to close");
      return false;
    }

    // Determine which buffer to restore based on tutorial mode
    let restore_buffer_idx = if self.tutorial_active && self.buffers.len() > 2 {
      // In tutorial mode with 3 buffers, restore to tutorial overlay (buffer 1)
      self.debug_log(
        "  Tutorial mode: removing command buffer, restoring tutorial overlay",
      );
      self.buffers.pop(); // Remove command buffer (index 2)
      1
    } else {
      // Normal mode, restore to main buffer (buffer 0)
      self.debug_log(
        "  Normal mode: removing split buffer, restoring main buffer",
      );
      if self.buffers.len() > 1 {
        self.buffers.pop(); // Remove split buffer
      }
      0
    };

    // Restore the appropriate buffer to full height
    if let Some(buffer) = self.buffers.get_mut(restore_buffer_idx) {
      buffer.viewport_height = self.height.saturating_sub(1);
      buffer.split_height = None;
    }

    self.active_buffer = restore_buffer_idx;
    self.active_pane = 0;
    self.view_mode = if restore_buffer_idx == 1 {
      ViewMode::Overlay
    } else {
      ViewMode::Normal
    };

    // Load the restored buffer state
    self.load_buffer_state(restore_buffer_idx);
    self.buffer_just_switched = true;

    self.debug_log("  Returned to normal view");
    self.debug_log("=== close_split complete ===");
    true
  }

  // Switch between split panes
  pub fn switch_split_pane(&mut self, pane: usize) {
    if self.view_mode != ViewMode::HorizontalSplit {
      return;
    }

    self.debug_log(&format!("=== switch_split_pane to {pane} ==="));
    self.debug_log(&format!(
      "  Current state: active_buffer={}, active_pane={}, buffers.len()={}",
      self.active_buffer,
      self.active_pane,
      self.buffers.len()
    ));

    // Save current buffer state
    self.save_current_buffer_state();

    // Determine the actual buffer index based on pane and tutorial mode
    let buffer_idx = if self.tutorial_active && self.buffers.len() > 2 {
      // In tutorial mode: pane 0 = buffer 1 (tutorial), pane 1 = buffer 2
      // (command)
      if pane == 0 { 1 } else { 2 }
    } else {
      // Normal mode: pane 0 = buffer 0 (main), pane 1 = buffer 1 (command)
      pane
    };

    self.debug_log(&format!("  Target: pane={pane}, buffer_idx={buffer_idx}"));

    // Update active buffer first, then pane
    self.active_buffer = buffer_idx;
    self.active_pane = pane;

    // Load new buffer state
    self.load_buffer_state(buffer_idx);

    // Log the loaded state
    self.debug_log(&format!(
      "  After switch: mode={:?}, lines={}, cursor=({},{}), offset={}",
      self.get_active_mode(),
      self.lines.len(),
      self.cursor_x,
      self.cursor_y,
      self.offset
    ));
    self.debug_log(&format!(
      "  Viewport height: {}",
      self.get_effective_viewport_height()
    ));

    // Force redraw to ensure cursor position is updated
    self.mark_dirty();
    self.cursor_moved = true;
    self.buffer_just_switched = true;

    self.debug_log(&format!("  Switched to pane {pane} (buffer {buffer_idx})"));
  }

  // Check if we're in split view
  #[allow(dead_code)]
  pub fn is_in_split_view(&self) -> bool {
    self.view_mode == ViewMode::HorizontalSplit
  }
}
