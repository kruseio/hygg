use crate::demo_registry::{get_demo_by_id, get_demo_content_by_id};
use crate::editor::core::{Editor, EditorMode, ViewMode};
use cli_pdf_to_text::PdfLineKind;
use std::time::Instant;

impl Editor {
  // Initialize demo mode with specified demo ID
  pub fn start_demo_mode(&mut self, demo_id: usize) {
    self.debug_log(&format!("Starting demo mode with ID: {demo_id}"));

    // Remember the document's real highlights before the demo touches them: the
    // demo runs `:h` and its cleanup rewrites the highlight file, so without
    // this a demo over a real document erases that document's highlights.
    self.demo_saved_highlights = Some(self.highlights.highlights.clone());

    // Load demo content if the document is empty or inappropriate for demo
    if self.lines.is_empty() || self.lines.len() < 10 {
      self.load_demo_content(demo_id);
    }

    if let Some(demo) = get_demo_by_id(demo_id) {
      self.debug_log(&format!(
        "Loaded demo {} with {} actions",
        demo_id,
        demo.actions.len()
      ));
      self.demo_script = Some(demo);
      self.demo_action_index = 0;
      self.demo_last_action_time = Some(Instant::now());
      self.demo_typing_char_index = 0;
      self.tutorial_demo_mode = true;
      self.tutorial_start_time = Some(Instant::now());

      // Don't use overlay mode - we want to show the actual document
      self.tutorial_active = false;
    } else {
      self.debug_log(&format!("Demo with ID {demo_id} not found"));
    }
  }

  // Load demo-specific content
  fn load_demo_content(&mut self, demo_id: usize) {
    let demo_text = get_demo_content_by_id(demo_id);
    // Apply justification to the demo content
    let justified_lines = cli_justify::justify(&demo_text, self.col);
    self.lines = justified_lines;
    self.line_kinds = vec![PdfLineKind::Text; self.lines.len()];
    self.buffers[0].lines = self.lines.clone();
    self.buffers[0].line_kinds = self.line_kinds.clone();
    self.total_lines = self.lines.len();
    self.offset = 0;
    self.cursor_y = self.height / 2;
    self.mark_dirty();
  }

  // Complete the demo
  pub(crate) fn complete_demo(&mut self) {
    self.debug_log("Completing demo mode - performing comprehensive cleanup");

    // IMPORTANT: We need to maintain state that signals demo completion for
    // exit The should_exit_after_demo() function checks:
    // !self.tutorial_demo_mode && self.demo_script.is_none() &&
    // self.demo_action_index > 0

    // Clear demo-specific state but preserve exit signal
    self.tutorial_demo_mode = false; // This MUST be false for exit check
    self.demo_script = None; // This MUST be None for exit check
    // Keep demo_action_index > 0 to signal demo completion for exit check
    // DO NOT RESET: self.demo_action_index = 0;

    self.demo_hint_text = None;
    self.demo_hint_until = None;
    self.demo_typing_char_index = 0;
    self.demo_pending_keys.clear();
    self.demo_last_action_time = None;

    // Restore the highlights the document had before the demo, rather than
    // clearing everything: the demo added its own via `:h`, but it ran on the
    // user's real document, so a blanket clear (which also rewrites the file)
    // took the user's highlights with it. Restoring the snapshot both drops the
    // demo's additions and rewrites the original set to disk.
    if let Some(saved) = self.demo_saved_highlights.take() {
      self.highlights.highlights = saved;
      if let Err(e) = crate::highlights::save_highlights(&self.highlights) {
        self.debug_log(&format!("Failed to restore demo highlights: {e}"));
      }
    } else {
      self.highlights.clear_all_highlights();
    }

    // Clear selection state
    self.clear_selection();

    // Clear yanked text
    self.editor_state.yank_buffer.clear();

    // Clear search state
    self.editor_state.search_query.clear();
    self.editor_state.current_match = None;

    // Clear bookmarks (optional - could preserve them)
    // self.marks.clear();

    // Clear any command output buffers (keep only main buffer)
    if self.buffers.len() > 1 {
      self.buffers.truncate(1);
    }

    // Reset to main buffer and normal view mode
    self.active_buffer = 0;
    self.active_pane = 0;
    self.view_mode = ViewMode::Normal;

    // Clear other navigation state
    self.number_prefix.clear();
    self.last_find_char = None;
    self.last_find_forward = false;
    self.last_find_till = false;
    self.previous_position = None;

    // Reset editor mode
    self.editor_state.mode = EditorMode::Normal;
    self.editor_state.command_buffer.clear();
    self.editor_state.command_cursor_pos = 0;

    // Sync buffer state
    if let Some(buffer) = self.buffers.get_mut(0) {
      buffer.selection_start = None;
      buffer.selection_end = None;
      buffer.search_query.clear();
      buffer.current_match = None;
      buffer.command_buffer.clear();
      buffer.command_cursor_pos = 0;
    }

    // Load the main buffer state
    self.load_buffer_state(0);

    // Force full redraw
    self.mark_dirty();
    self.force_clear = true;

    self.debug_log("Demo cleanup complete - demo should exit now");
  }

  // Check if demo should exit
  pub fn should_exit_after_demo(&self) -> bool {
    // If we started in demo mode and it's now complete, exit
    let should_exit = !self.tutorial_demo_mode
      && self.demo_script.is_none()
      && self.demo_action_index > 0;

    if self.demo_action_index > 0 {
      self.debug_log(&format!(
                "should_exit_after_demo: tutorial_demo_mode={}, demo_script={}, demo_action_index={}, result={}",
                self.tutorial_demo_mode,
                self.demo_script.is_some(),
                self.demo_action_index,
                should_exit
            ));
    }

    should_exit
  }
}
