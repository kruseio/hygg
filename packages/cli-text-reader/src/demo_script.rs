use crate::demo_components::get_component;
use crossterm::event::{KeyCode, KeyModifiers};
use std::time::Duration;

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum DemoAction {
  // Wait for a duration
  Wait(Duration),
  // Press a single key
  Key(KeyCode),
  // Press a key with modifiers
  KeyWithModifiers(KeyCode, KeyModifiers),
  // Type a string character by character
  TypeString(String, Duration), // string, delay between chars
  // Show a subtitle/hint
  ShowHint(String, Duration), // hint text, display duration
  // Mark a checkpoint (for debugging)
  Checkpoint(String),
  // Execute a vim motion (e.g., "vip", "viw", "yy")
  VimMotion(String),
}

#[allow(dead_code)]
pub struct DemoScript {
  pub actions: Vec<DemoAction>,
}

impl DemoScript {
  // Build demo from component IDs
  pub fn from_components(component_ids: &[&str]) -> Self {
    let mut actions = vec![];

    for id in component_ids {
      if let Some(component) = get_component(id) {
        actions.extend(component.actions);
      } else {
        eprintln!("Warning: Unknown demo component ID: {id}");
      }
    }

    Self { actions }
  }

  pub fn marketing_demo() -> Self {
    Self::from_components(&[
      "intro_message",
      "select_paragraph",
      "highlight_selection",
      "execute_ls",
      "search_cargo",
      "yank_line",
      "execute_cat_with_paste",
      // "final_message",
    ])
  }

  pub fn speed_demo() -> Self {
    Self::from_components(&[
      "intro_message",
      "basic_navigation",
      "word_navigation",
      "select_paragraph",
      "highlight_selection",
      "final_message_short",
    ])
  }

  pub fn power_user_demo() -> Self {
    Self::from_components(&[
      "intro_message",
      "paragraph_navigation",
      "search_navigation",
      "visual_char_mode",
      "yank_selection",
      "execute_ls",
      "yank_and_execute",
      "execute_grep",
      "final_message",
    ])
  }

  pub fn minimal_demo() -> Self {
    Self::from_components(&[
      "intro_message",
      "basic_navigation",
      "select_word",
      "highlight_selection",
      "clear_highlights",
      "final_message_short",
    ])
  }

  pub fn workflow_demo() -> Self {
    Self::from_components(&[
      "intro_message",
      "basic_navigation",
      "visual_char_mode",
      "yank_selection",
      "execute_cat",
      "select_line",
      "highlight_selection",
      "final_message",
    ])
  }

  #[allow(dead_code)]
  pub fn beginner_tutorial() -> Self {
    use DemoAction::*;

    let actions = vec![
      // Welcome
      ShowHint(
        "Welcome to hygg! Let's learn the basics.".to_string(),
        Duration::from_millis(3000),
      ),
      Wait(Duration::from_millis(1000)),
      // Basic movement
      ShowHint(
        "Use j/k to move down/up (try it now!)".to_string(),
        Duration::from_millis(5000),
      ),
      Checkpoint("Waiting for user to practice j/k".to_string()),
      // More to be added based on user interaction
    ];

    Self { actions }
  }

  // Test script for the interactive tutorial
  #[allow(dead_code)]
  pub fn tutorial_test() -> Self {
    use DemoAction::*;

    let actions = vec![
      // Wait for initial load
      Wait(Duration::from_millis(500)),
      // Step 1: Welcome - press 'j' to move down
      Wait(Duration::from_millis(500)),
      Key(KeyCode::Char('j')),
      Wait(Duration::from_millis(500)),
      // Type :next to continue
      Key(KeyCode::Char(':')),
      Wait(Duration::from_millis(100)),
      Key(KeyCode::Char('n')),
      Key(KeyCode::Char('e')),
      Key(KeyCode::Char('x')),
      Key(KeyCode::Char('t')),
      Key(KeyCode::Enter),
      Wait(Duration::from_millis(500)),
      // Step 2: Visual Selection - press 'v' and move to select, then 'y' to
      // yank
      Key(KeyCode::Char('v')),
      Wait(Duration::from_millis(200)),
      Key(KeyCode::Char('w')),
      Wait(Duration::from_millis(200)),
      Key(KeyCode::Char('y')),
      Wait(Duration::from_millis(500)),
      // Type :next
      Key(KeyCode::Char(':')),
      Wait(Duration::from_millis(100)),
      Key(KeyCode::Char('n')),
      Key(KeyCode::Char('e')),
      Key(KeyCode::Char('x')),
      Key(KeyCode::Char('t')),
      Key(KeyCode::Enter),
      Wait(Duration::from_millis(500)),
      // Step 3: Text Objects - vip to select paragraph, then :h to highlight
      VimMotion("vip".to_string()),
      Wait(Duration::from_millis(500)),
      Key(KeyCode::Char(':')),
      Wait(Duration::from_millis(100)),
      Key(KeyCode::Char('h')),
      Key(KeyCode::Enter),
      Wait(Duration::from_millis(500)),
      // Type :next
      Key(KeyCode::Char(':')),
      Wait(Duration::from_millis(100)),
      Key(KeyCode::Char('n')),
      Key(KeyCode::Char('e')),
      Key(KeyCode::Char('x')),
      Key(KeyCode::Char('t')),
      Key(KeyCode::Enter),
      Wait(Duration::from_millis(500)),
      // Step 4: Search - search for "special"
      Key(KeyCode::Char('/')),
      Wait(Duration::from_millis(200)),
      Key(KeyCode::Char('s')),
      Key(KeyCode::Char('p')),
      Key(KeyCode::Char('e')),
      Key(KeyCode::Char('c')),
      Key(KeyCode::Char('i')),
      Key(KeyCode::Char('a')),
      Key(KeyCode::Char('l')),
      Key(KeyCode::Enter),
      Wait(Duration::from_millis(500)),
      // Type :next
      Key(KeyCode::Char(':')),
      Wait(Duration::from_millis(100)),
      Key(KeyCode::Char('n')),
      Key(KeyCode::Char('e')),
      Key(KeyCode::Char('x')),
      Key(KeyCode::Char('t')),
      Key(KeyCode::Enter),
      Wait(Duration::from_millis(500)),
      // Step 5: Bookmarks - set bookmark 'a' with 'ma'
      Key(KeyCode::Char('m')),
      Wait(Duration::from_millis(200)),
      Key(KeyCode::Char('a')),
      Wait(Duration::from_millis(500)),
      // Type :next
      Key(KeyCode::Char(':')),
      Wait(Duration::from_millis(100)),
      Key(KeyCode::Char('n')),
      Key(KeyCode::Char('e')),
      Key(KeyCode::Char('x')),
      Key(KeyCode::Char('t')),
      Key(KeyCode::Enter),
      Wait(Duration::from_millis(500)),
      // Step 6: Command Execution - run :!echo hello world
      Key(KeyCode::Char(':')),
      Wait(Duration::from_millis(100)),
      Key(KeyCode::Char('!')),
      Key(KeyCode::Char('e')),
      Key(KeyCode::Char('c')),
      Key(KeyCode::Char('h')),
      Key(KeyCode::Char('o')),
      Key(KeyCode::Char(' ')),
      Key(KeyCode::Char('h')),
      Key(KeyCode::Char('e')),
      Key(KeyCode::Char('l')),
      Key(KeyCode::Char('l')),
      Key(KeyCode::Char('o')),
      Key(KeyCode::Char(' ')),
      Key(KeyCode::Char('w')),
      Key(KeyCode::Char('o')),
      Key(KeyCode::Char('r')),
      Key(KeyCode::Char('l')),
      Key(KeyCode::Char('d')),
      Key(KeyCode::Enter),
      Wait(Duration::from_millis(1000)), // Wait for command execution
      // Type :next
      Key(KeyCode::Char(':')),
      Wait(Duration::from_millis(100)),
      Key(KeyCode::Char('n')),
      Key(KeyCode::Char('e')),
      Key(KeyCode::Char('x')),
      Key(KeyCode::Char('t')),
      Key(KeyCode::Enter),
      Wait(Duration::from_millis(500)),
      // Final step: Press Enter to complete tutorial
      Key(KeyCode::Enter),
      Wait(Duration::from_millis(1000)),
    ];

    Self { actions }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_from_components() {
    let demo = DemoScript::from_components(&["intro_message", "final_message"]);
    assert!(!demo.actions.is_empty());
  }

  #[test]
  fn test_marketing_demo_has_correct_components() {
    let demo = DemoScript::marketing_demo();
    assert!(!demo.actions.is_empty());
  }

  #[test]
  fn test_from_components_with_invalid_id() {
    // Should handle invalid component IDs gracefully
    let demo = DemoScript::from_components(&[
      "intro_message",
      "invalid_component",
      "final_message",
    ]);
    // Should still have actions from valid components
    assert!(!demo.actions.is_empty());
  }
}
