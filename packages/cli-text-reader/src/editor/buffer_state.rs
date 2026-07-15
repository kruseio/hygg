use super::core::{BufferState, Editor};
use cli_pdf_to_text::PdfLineKind;

impl Editor {
  // Save current editor state to the active buffer
  pub fn save_current_buffer_state(&mut self) {
    let active_idx = self.active_buffer;
    if let Some(buffer) = self.buffers.get_mut(active_idx) {
      // Save position and display state
      buffer.lines = self.lines.clone();
      buffer.line_kinds = self.line_kinds.clone();
      buffer.offset = self.offset;
      buffer.cursor_x = self.cursor_x;
      buffer.cursor_y = self.cursor_y;

      // Save search state
      buffer.search_query = self.editor_state.search_query.clone();
      buffer.current_match = self.editor_state.current_match;

      // Save selection state
      buffer.selection_start = self.editor_state.selection_start;
      buffer.selection_end = self.editor_state.selection_end;

      // Save mode and command state
      buffer.mode = self.editor_state.mode.clone();
      buffer.command_buffer = self.editor_state.command_buffer.clone();
      buffer.command_cursor_pos = self.editor_state.command_cursor_pos;

      // Extract values for logging to avoid borrow checker issues
      let offset = buffer.offset;
      let cursor_x = buffer.cursor_x;
      let cursor_y = buffer.cursor_y;
      let mode = buffer.mode.clone();
      let command_buffer = buffer.command_buffer.clone();

      self.debug_log(&format!(
        "Saved buffer {active_idx} state: offset={offset}, cursor=({cursor_x},{cursor_y}), mode={mode:?}, cmd_buf='{command_buffer}'"
      ));
    } else {
      self.debug_log(&format!(
        "ERROR: Cannot save state for buffer {active_idx}"
      ));
    }
  }

  // Load buffer state into the editor
  pub fn load_buffer_state(&mut self, buffer_idx: usize) {
    self
      .debug_log(&format!("=== load_buffer_state for buffer {buffer_idx} ==="));
    self.debug_log(&format!(
      "  Total buffers: {}, active: {}",
      self.buffers.len(),
      self.active_buffer
    ));

    if let Some(buffer) = self.buffers.get(buffer_idx) {
      self.debug_log(&format!(
        "  Buffer found: lines={}, viewport_height={}, split_height={:?}",
        buffer.lines.len(),
        buffer.viewport_height,
        buffer.split_height
      ));

      // Enhanced empty buffer handling with more informative placeholder
      if buffer.lines.is_empty() {
        self.debug_log(
          "  WARNING: Buffer has no lines, using enhanced placeholder",
        );
        self.lines = vec![
          "Buffer is empty".to_string(),
          "".to_string(),
          "Press :q to exit".to_string(),
        ];
        self.line_kinds = vec![PdfLineKind::Text; self.lines.len()];
        self.total_lines = self.lines.len();
      } else {
        // Load document content
        self.lines = buffer.lines.clone();
        self.line_kinds = buffer.line_kinds.clone();
        if self.line_kinds.len() != self.lines.len() {
          self.line_kinds = vec![PdfLineKind::Text; self.lines.len()];
        }
        self.total_lines = buffer.lines.len();
      }

      // Safer debug logging with additional validation
      let first_line_preview = if self.lines.is_empty() {
        self.debug_log("  WARNING: self.lines is empty after assignment!");
        "EMPTY".to_string()
      } else {
        match self.lines.first() {
          Some(line) => {
            if line.is_empty() {
              "EMPTY_LINE".to_string()
            } else {
              let preview_len = line.len().min(50);
              match line.get(..preview_len) {
                Some(slice) => slice.to_string(),
                None => {
                  self.debug_log(&format!("  ERROR: Cannot slice line of length {} with preview_len {}", line.len(), preview_len));
                  "SLICE_ERROR".to_string()
                }
              }
            }
          }
          None => {
            self.debug_log("  ERROR: self.lines.first() returned None despite non-empty check!");
            "NONE".to_string()
          }
        }
      };

      self.debug_log(&format!(
        "  Loaded lines: count={}, first_line={:?}",
        self.lines.len(),
        first_line_preview
      ));

      // Load position and display state
      self.offset = buffer.offset;
      self.cursor_x = buffer.cursor_x;
      self.cursor_y = buffer.cursor_y;

      // Load search state
      self.editor_state.search_query = buffer.search_query.clone();
      self.editor_state.current_match = buffer.current_match;

      // Load selection state
      self.editor_state.selection_start = buffer.selection_start;
      self.editor_state.selection_end = buffer.selection_end;

      // Load mode and command state
      self.editor_state.mode = buffer.mode.clone();
      self.editor_state.command_buffer = buffer.command_buffer.clone();
      self.editor_state.command_cursor_pos = buffer.command_cursor_pos;

      // Calculate viewport height with enhanced validation
      let base_viewport_height =
        if self.view_mode == super::core::ViewMode::HorizontalSplit {
          buffer.viewport_height
        } else {
          self.height.saturating_sub(1)
        };

      // Ensure viewport_height is reasonable
      let viewport_height =
        base_viewport_height.max(3).min(self.height.saturating_sub(1));
      self.debug_log(&format!("  Viewport height: base={base_viewport_height}, adjusted={viewport_height}"));

      // Enhanced cursor validation with better edge case handling
      // First ensure offset is valid
      if self.offset >= self.total_lines && self.total_lines > 0 {
        let old_offset = self.offset;
        self.offset = self.total_lines.saturating_sub(viewport_height);
        self.debug_log(&format!(
          "  WARNING: Reset offset from {} to {} (total_lines={})",
          old_offset, self.offset, self.total_lines
        ));
      }

      // Validate cursor_y within viewport and document bounds
      let max_cursor_y = viewport_height
        .saturating_sub(1)
        .min(self.total_lines.saturating_sub(self.offset + 1));
      if self.cursor_y > max_cursor_y {
        let old_cursor_y = self.cursor_y;
        self.cursor_y = max_cursor_y;
        self.debug_log(&format!(
          "  WARNING: Adjusted cursor_y from {} to {} (max={})",
          old_cursor_y, self.cursor_y, max_cursor_y
        ));
      }

      // Validate cursor_x is within line bounds with better handling
      let current_line_idx = self.offset + self.cursor_y;
      if current_line_idx < self.lines.len() {
        let line_len = self.lines[current_line_idx].len();
        if self.cursor_x > line_len {
          let old_cursor_x = self.cursor_x;
          self.cursor_x = line_len;
          self.debug_log(&format!(
            "  WARNING: Adjusted cursor_x from {} to {} (line {} len={})",
            old_cursor_x, self.cursor_x, current_line_idx, line_len
          ));
        }
      } else {
        // If we're somehow beyond the document, reset to safe position
        self.debug_log(&format!("  ERROR: Line index {current_line_idx} out of bounds, resetting cursor"));
        self.cursor_y = 0;
        self.cursor_x = 0;
      }

      self.debug_log(&format!(
        "  Loaded state: lines={}, offset={}, cursor=({},{}), mode={:?}, cmd_buf='{}'",
        self.lines.len(), self.offset, self.cursor_x, self.cursor_y,
        self.editor_state.mode, self.editor_state.command_buffer
      ));
      self.debug_log(&format!(
        "  is_split_buffer={}, split_position={:?}",
        buffer.is_split_buffer, buffer.split_position
      ));
    } else {
      self.debug_log(&format!(
        "  ERROR: Buffer {buffer_idx} not found! Creating safe defaults"
      ));
      // Enhanced safe defaults if buffer not found
      self.lines = vec![
        "Error: Buffer not found".to_string(),
        "".to_string(),
        "This is likely a bug. Press :q to exit".to_string(),
      ];
      self.line_kinds = vec![PdfLineKind::Text; self.lines.len()];
      self.total_lines = self.lines.len();
      self.offset = 0;
      self.cursor_x = 0;
      self.cursor_y = 0;

      // Reset editor state to safe defaults
      self.editor_state.mode = super::core::EditorMode::Normal;
      self.editor_state.command_buffer.clear();
      self.editor_state.command_cursor_pos = 0;
      self.editor_state.search_query.clear();
      self.editor_state.current_match = None;
      self.editor_state.selection_start = None;
      self.editor_state.selection_end = None;
    }

    // Final safety check
    if self.lines.is_empty() {
      self.debug_log(
        "  CRITICAL: Lines still empty after load, adding final fallback",
      );
      self.lines = vec!["[Empty]".to_string()];
      self.line_kinds = vec![PdfLineKind::Text];
      self.total_lines = 1;
    }

    self.debug_log("=== load_buffer_state complete ===");
  }

  // Get the active buffer
  #[allow(dead_code)]
  pub fn get_active_buffer(&self) -> Option<&BufferState> {
    self.buffers.get(self.active_buffer)
  }

  // Get the active buffer mutably
  #[allow(dead_code)]
  pub fn get_active_buffer_mut(&mut self) -> Option<&mut BufferState> {
    self.buffers.get_mut(self.active_buffer)
  }

  // Center cursor within the active buffer's viewport
  #[allow(dead_code)]
  pub fn center_cursor_in_buffer(&mut self) {
    if let Some(buffer) = self.get_active_buffer() {
      let viewport_height = buffer.viewport_height;
      let center = viewport_height / 2;
      let current_line = self.offset + self.cursor_y;

      // Calculate new offset for centering within viewport
      let new_offset = current_line.saturating_sub(center);
      let max_offset = self.total_lines.saturating_sub(viewport_height);
      self.offset = new_offset.min(max_offset);

      // Update cursor_y relative to new offset, ensuring it stays within
      // viewport
      self.cursor_y = if current_line >= self.offset {
        let relative_pos = current_line - self.offset;
        // Ensure cursor_y doesn't exceed viewport bounds
        relative_pos.min(viewport_height.saturating_sub(1))
      } else {
        0
      };

      self.debug_log(&format!(
        "Centered cursor in buffer: line={}, offset={}, cursor_y={}, viewport_height={}",
        current_line, self.offset, self.cursor_y, viewport_height
      ));
    }
  }
}
