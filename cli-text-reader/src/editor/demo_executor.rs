use super::core::{Editor, EditorMode, ViewMode};
use crate::demo_registry::{get_demo_by_id, get_demo_content_by_id};
use crate::demo_script::{DemoAction, DemoScript};
use cli_pdf_to_text::PdfLineKind;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::time::{Duration, Instant};

impl Editor {
  // Initialize demo mode with specified demo ID
  pub fn start_demo_mode(&mut self, demo_id: usize) {
    self.debug_log(&format!("Starting demo mode with ID: {demo_id}"));

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

  // Check if it's time to execute the next demo action
  pub fn check_demo_progress(&mut self) -> Option<KeyEvent> {
    if !self.tutorial_demo_mode {
      return None;
    }

    // First check if we have pending keys from a vim motion
    if !self.demo_pending_keys.is_empty() {
      self.debug_log(&format!(
        "Demo: Processing pending key {:?}",
        self.demo_pending_keys[0]
      ));
      return Some(self.demo_pending_keys.remove(0));
    }

    // Check if demo is complete
    let Some(demo_script) = &self.demo_script else {
      self.debug_log("No demo script found");
      return None;
    };
    let actions_len = demo_script.actions.len();

    self.debug_log(&format!(
      "Demo progress: action {} of {}",
      self.demo_action_index, actions_len
    ));

    if self.demo_action_index >= actions_len {
      self.debug_log(&format!(
        "Demo script complete - action_index {} >= actions_len {}",
        self.demo_action_index, actions_len
      ));
      self.complete_demo();
      // Force immediate redraw so exit check happens right away
      self.mark_dirty();
      return None;
    }

    // Don't automatically clear hints - they will be replaced when a new hint
    // is shown or cleared when the demo completes

    let last_action_time = self.demo_last_action_time?;

    // Clone the action to avoid borrow issues
    let current_action =
      self.demo_script.as_ref()?.actions[self.demo_action_index].clone();

    match current_action {
      DemoAction::Wait(duration) => {
        if Instant::now() >= last_action_time + duration {
          self.debug_log("Demo: Wait complete, moving to next action");
          self.demo_action_index += 1;
          self.demo_typing_char_index = 0; // Reset typing index
          self.demo_last_action_time = Some(Instant::now());
          self.mark_dirty();
          return self.check_demo_progress(); // Check next action immediately
        }
      }

      DemoAction::Key(key_code) => {
        self.debug_log(&format!("Demo: Simulating key press: {key_code:?}"));

        self.demo_action_index += 1;
        self.demo_typing_char_index = 0; // Reset typing index
        self.demo_last_action_time = Some(Instant::now());

        return Some(KeyEvent::new(key_code, KeyModifiers::empty()));
      }

      DemoAction::KeyWithModifiers(key_code, modifiers) => {
        self.debug_log(&format!(
          "Demo: Simulating key with modifiers: {key_code:?} + {modifiers:?}"
        ));

        self.demo_action_index += 1;
        self.demo_typing_char_index = 0; // Reset typing index
        self.demo_last_action_time = Some(Instant::now());

        return Some(KeyEvent::new(key_code, modifiers));
      }

      DemoAction::TypeString(text, char_delay) => {
        // Check if it's time to type the next character
        let elapsed = Instant::now().duration_since(last_action_time);

        if elapsed >= char_delay {
          // Type the current character
          if self.demo_typing_char_index < text.chars().count() {
            let ch = text.chars().nth(self.demo_typing_char_index).unwrap();
            self.debug_log(&format!(
              "Demo: Typing character {} of {}: '{}'",
              self.demo_typing_char_index + 1,
              text.chars().count(),
              ch
            ));

            // Advance to next character
            self.demo_typing_char_index += 1;
            self.demo_last_action_time = Some(Instant::now());

            return Some(KeyEvent::new(
              KeyCode::Char(ch),
              KeyModifiers::empty(),
            ));
          } else {
            // String complete, move to next action
            self.debug_log(&format!("Demo: Finished typing: {text}"));
            self.demo_action_index += 1;
            self.demo_typing_char_index = 0; // Reset for next TypeString action
            self.demo_last_action_time = Some(Instant::now());
            return self.check_demo_progress();
          }
        }
        // Not time to type next character yet
      }

      DemoAction::ShowHint(hint, duration) => {
        self.debug_log(&format!("Demo: Showing hint: {hint}"));
        // Only mark dirty if the hint actually changed
        let hint_changed = self.demo_hint_text.as_ref() != Some(&hint);
        self.demo_hint_text = Some(hint.clone());
        self.demo_hint_until = Some(Instant::now() + duration);

        self.demo_action_index += 1;
        self.demo_typing_char_index = 0; // Reset typing index
        self.demo_last_action_time = Some(Instant::now());
        if hint_changed {
          self.mark_dirty();
        }
        // Don't recurse - let the main loop handle timing
        return None;
      }

      DemoAction::Checkpoint(message) => {
        self.debug_log(&format!("Demo checkpoint: {message}"));
        self.demo_action_index += 1;
        self.demo_typing_char_index = 0; // Reset typing index
        self.demo_last_action_time = Some(Instant::now());
        return self.check_demo_progress();
      }

      DemoAction::VimMotion(motion) => {
        self.debug_log(&format!("Demo: Executing vim motion: {motion}"));

        // Convert the motion string into key events
        let key_events = self.parse_vim_motion(&motion);

        // Store the key events for processing
        self.demo_pending_keys = key_events;

        self.demo_action_index += 1;
        self.demo_typing_char_index = 0; // Reset typing index
        self.demo_last_action_time = Some(Instant::now());

        // Process the first key immediately
        if !self.demo_pending_keys.is_empty() {
          return Some(self.demo_pending_keys.remove(0));
        }
      }
    }

    None
  }

  // Complete the demo
  fn complete_demo(&mut self) {
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

    // Clear all highlights created during demo
    self.highlights.clear_all_highlights();

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

  // Parse a vim motion string into a sequence of key events
  pub fn parse_vim_motion(&self, motion: &str) -> Vec<KeyEvent> {
    let mut events = Vec::new();

    for ch in motion.chars() {
      events.push(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::empty()));
    }

    events
  }
}
