use crate::demo_script::DemoAction;
use crate::editor::core::Editor;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::time::{Duration, Instant};

impl Editor {
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

  // Parse a vim motion string into a sequence of key events
  pub fn parse_vim_motion(&self, motion: &str) -> Vec<KeyEvent> {
    let mut events = Vec::new();

    for ch in motion.chars() {
      events.push(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::empty()));
    }

    events
  }
}
