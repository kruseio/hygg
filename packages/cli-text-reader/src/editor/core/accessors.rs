use super::super::char_boundary::{CursorCol, Pos};
use super::{Editor, EditorMode};

impl Editor {
  // The cursor's position in the document: its line, and its byte column
  pub fn get_cursor_position(&self) -> Pos {
    // Calculate the correct line index based on the cursor's position
    // This ensures we get the line currently being displayed under the cursor
    let line_idx = self.offset + self.cursor_y;

    // Make sure we don't exceed the document boundaries
    let line_idx = line_idx.min(self.lines.len().saturating_sub(1));

    Pos::new(line_idx, self.cursor_x.col())
  }

  // Settle `cursor_x` (a byte offset) onto the start of a grapheme cluster of
  // the current line, no further than its last one. Anything that leaves a
  // stale, mid-character or mid-grapheme column behind (a buffer switch, lines
  // replaced under the cursor, a search match on a combining mark or inside a
  // ZWJ emoji) is corrected here before the next draw slices the line with it
  // (issue #99). No-op when the cursor line does not exist.
  pub(crate) fn normalize_cursor_column(&mut self) {
    if let Some(line) = self.lines.get(self.offset + self.cursor_y) {
      self.cursor_x = CursorCol::clamped(line, self.cursor_x.col());
    }
  }

  // Debug logging helper
  pub fn debug_log(&self, message: &str) {
    crate::debug::debug_log("editor", message);
  }

  pub fn debug_log_event(&self, module: &str, event: &str, details: &str) {
    crate::debug::debug_log_event(module, event, details);
  }

  pub fn debug_log_state(
    &self,
    module: &str,
    state_name: &str,
    state_value: &str,
  ) {
    crate::debug::debug_log_state(module, state_name, state_value);
  }

  pub fn debug_log_error(&self, error: &str) {
    crate::debug::debug_log_error("editor", error);
  }

  // Calculate dimensions for display
  #[allow(dead_code)]
  pub fn calculate_dimensions(&self) -> usize {
    // Always use full height minus status line
    self.height.saturating_sub(1)
  }

  // Helper methods to access active buffer's mode and command state
  pub fn get_active_mode(&self) -> EditorMode {
    if let Some(buffer) = self.buffers.get(self.active_buffer) {
      buffer.mode.clone()
    } else {
      // Fallback to editor state mode during migration
      self.editor_state.mode.clone()
    }
  }

  pub fn set_active_mode(&mut self, mode: EditorMode) {
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.mode = mode.clone();
    }
    // Also update editor state during migration
    self.editor_state.mode = mode;
  }

  pub fn get_active_command_buffer(&self) -> &str {
    if let Some(buffer) = self.buffers.get(self.active_buffer) {
      &buffer.command_buffer
    } else {
      // Fallback to editor state during migration
      &self.editor_state.command_buffer
    }
  }

  #[allow(dead_code)]
  pub fn get_active_command_buffer_mut(&mut self) -> &mut String {
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      &mut buffer.command_buffer
    } else {
      // Fallback to editor state during migration
      &mut self.editor_state.command_buffer
    }
  }

  pub fn get_active_command_cursor_pos(&self) -> usize {
    if let Some(buffer) = self.buffers.get(self.active_buffer) {
      buffer.command_cursor_pos
    } else {
      // Fallback to editor state during migration
      self.editor_state.command_cursor_pos
    }
  }

  #[allow(dead_code)]
  pub fn set_active_command_cursor_pos(&mut self, pos: usize) {
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.command_cursor_pos = pos;
    }
    // Also update editor state during migration
    self.editor_state.command_cursor_pos = pos;
  }

  // Save bookmarks to file
  pub fn save_bookmarks(&self) {
    use crate::bookmarks::save_bookmarks;
    if let Err(e) = save_bookmarks(self.document_hash, &self.marks) {
      self.debug_log_error(&format!("Failed to save bookmarks: {e}"));
    }
  }

  // Save highlights to file
  pub fn save_highlights(&self) {
    use crate::highlights::save_highlights;
    if let Err(e) = save_highlights(&self.highlights) {
      self.debug_log_error(&format!("Failed to save highlights: {e}"));
    }
  }

  // Mark editor as needing redraw
  pub fn mark_dirty(&mut self) {
    self.needs_redraw = true;
  }

  // Check if redraw is needed and reset flag
  pub fn check_needs_redraw(&mut self) -> bool {
    let needs = self.needs_redraw;
    self.needs_redraw = false;
    needs
  }

  // Get the effective viewport height for the current buffer
  pub fn get_effective_viewport_height(&self) -> usize {
    if self.view_mode == super::ViewMode::HorizontalSplit {
      if let Some(buffer) = self.buffers.get(self.active_buffer) {
        buffer.viewport_height
      } else {
        self.height.saturating_sub(1)
      }
    } else {
      self.height.saturating_sub(1)
    }
  }
}

#[cfg(test)]
mod tests {
  use super::Editor;
  use crate::editor::char_boundary::CursorCol;

  #[test]
  fn normalize_cursor_column_snaps_onto_a_char_start() {
    // "La canción está aquí": 'ó' is bytes 8..10, 'í' (the last char) 21..23.
    let mut editor =
      Editor::new(vec!["La canción está aquí".to_string(), "".to_string()], 80);
    editor.offset = 0;
    editor.cursor_y = 0;

    editor.cursor_x = CursorCol::lit(9);
    editor.normalize_cursor_column();
    assert_eq!(editor.cursor_x.raw(), 8);

    editor.cursor_x = CursorCol::lit(10);
    editor.normalize_cursor_column();
    assert_eq!(editor.cursor_x.raw(), 10);

    editor.cursor_x = CursorCol::lit(22);
    editor.normalize_cursor_column();
    assert_eq!(editor.cursor_x.raw(), 21);

    editor.cursor_x = CursorCol::lit(99);
    editor.normalize_cursor_column();
    assert_eq!(editor.cursor_x.raw(), 21);

    editor.cursor_y = 1;
    editor.cursor_x = CursorCol::lit(3);
    editor.normalize_cursor_column();
    assert_eq!(editor.cursor_x.raw(), 0);

    // No such line: leave the column alone.
    editor.cursor_y = 5;
    editor.cursor_x = CursorCol::lit(3);
    editor.normalize_cursor_column();
    assert_eq!(editor.cursor_x.raw(), 3);
  }

  #[test]
  fn normalize_cursor_column_snaps_onto_a_grapheme_start() {
    // "ae\u{301}" a 0, e 1, U+0301 2..4. "x👩‍💻" x 0, 👩 1..5, ZWJ 5..8,
    // 💻 8..12. "ที่" is one grapheme of three chars, 9 bytes. "🇪🇸🇪🇸" is two
    // 8-byte flags.
    let lines = ["ae\u{301}", "x👩\u{200d}💻", "ที่", "🇪🇸🇪🇸", "कि"];
    let mut editor =
      Editor::new(lines.iter().map(|l| l.to_string()).collect(), 80);
    editor.offset = 0;
    for (row, x, expected) in [
      // On the combining accent: back onto its 'e'.
      (0, 2, 1),
      // Past the end: the last grapheme start, not the accent's char start.
      (0, 99, 1),
      // ZWJ and the 💻 tail both belong to the 👩 grapheme.
      (1, 5, 1),
      (1, 8, 1),
      (1, 99, 1),
      // The tone mark and the vowel sign of ที่.
      (2, 3, 0),
      (2, 6, 0),
      (2, 99, 0),
      // Inside the second flag, and between the two regional indicators of
      // the first.
      (3, 12, 8),
      (3, 4, 0),
      (3, 99, 8),
      // The Devanagari vowel sign 'ि' (bytes 3..6) belongs to 'क'.
      (4, 3, 0),
    ] {
      editor.cursor_y = row;
      editor.cursor_x = CursorCol::lit(x);
      editor.normalize_cursor_column();
      assert_eq!(editor.cursor_x.raw(), expected, "{:?} at {x}", lines[row]);
    }
  }

  #[test]
  fn loading_buffer_state_snaps_a_mid_char_cursor() {
    let mut editor = Editor::new(vec!["canción €5".to_string()], 80);
    editor.offset = 0;
    editor.cursor_y = 0;
    // 'ó' is bytes 5..7; '€' is 9..12.
    editor.buffers[0].offset = 0;
    editor.buffers[0].cursor_y = 0;
    editor.buffers[0].cursor_x = CursorCol::lit(6);
    editor.load_buffer_state(0);
    assert_eq!(editor.cursor_x.raw(), 5);

    editor.buffers[0].cursor_x = CursorCol::lit(11);
    editor.load_buffer_state(0);
    assert_eq!(editor.cursor_x.raw(), 9);

    editor.buffers[0].cursor_x = CursorCol::lit(99);
    editor.load_buffer_state(0);
    assert_eq!(editor.cursor_x.raw(), 12);
  }
}
