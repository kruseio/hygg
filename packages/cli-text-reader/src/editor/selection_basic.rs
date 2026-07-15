use super::core::Editor;

impl Editor {
  // Start selection at current cursor position
  pub fn start_selection(&mut self) {
    let pos = self.get_cursor_position();
    self.editor_state.selection_start = Some(pos);
    self.editor_state.selection_end = Some(pos);
    // Also update active buffer's selection
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.selection_start = Some(pos);
      buffer.selection_end = Some(pos);
    }
  }

  // Update selection end to current cursor position
  pub fn update_selection(&mut self) {
    let pos = self.get_cursor_position();
    self.editor_state.selection_end = Some(pos);
    // Also update active buffer's selection
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.selection_end = Some(pos);
    }
  }

  // Save current visual selection for gv command
  pub fn save_visual_selection(&mut self) {
    if let Some(buffer) = self.buffers.get(self.active_buffer)
      && buffer.selection_start.is_some()
      && buffer.selection_end.is_some()
    {
      let mode = self.get_active_mode();
      self.debug_log(&format!(
        "Saving visual selection: start={:?}, end={:?}, mode={:?}",
        buffer.selection_start, buffer.selection_end, mode
      ));
      self.editor_state.last_visual_start = buffer.selection_start;
      self.editor_state.last_visual_end = buffer.selection_end;
      self.editor_state.last_visual_mode = Some(mode);
    }
  }

  // Clear current selection
  pub fn clear_selection(&mut self) {
    self.editor_state.selection_start = None;
    self.editor_state.selection_end = None;
    self.editor_state.visual_selection_active = false;
    self.editor_state.previous_visual_mode = None;
    // Also clear active buffer's selection
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.selection_start = None;
      buffer.selection_end = None;
    }
  }

  // Restore last visual selection (for gv command)
  pub fn restore_visual_selection(&mut self) {
    self.debug_log(&format!(
      "Attempting to restore visual selection: start={:?}, end={:?}, mode={:?}",
      self.editor_state.last_visual_start,
      self.editor_state.last_visual_end,
      self.editor_state.last_visual_mode
    ));

    if let (Some(start), Some(end), Some(mode)) = (
      self.editor_state.last_visual_start,
      self.editor_state.last_visual_end,
      self.editor_state.last_visual_mode.clone(),
    ) {
      self.debug_log(&format!(
        "Restoring visual selection: mode={mode:?}, start={start:?}, end={end:?}"
      ));

      // Enter the appropriate visual mode
      self.set_active_mode(mode);

      // Restore the selection
      self.editor_state.selection_start = Some(start);
      self.editor_state.selection_end = Some(end);

      // Also update active buffer's selection
      if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
        buffer.selection_start = Some(start);
        buffer.selection_end = Some(end);
      }

      // Move cursor to the end of the selection
      let (line, col) = end;
      self.move_to_position(line, col);
    } else {
      self.debug_log("No previous visual selection to restore");
    }
  }
}
