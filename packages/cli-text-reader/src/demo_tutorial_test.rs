use crate::demo_script::{DemoAction, DemoScript};
use crossterm::event::KeyCode;
use std::time::Duration;

// Create a demo script that tests the entire interactive tutorial
#[allow(dead_code)]
#[allow(clippy::vec_init_then_push)]
pub fn create_tutorial_test_script() -> DemoScript {
  let mut actions = Vec::new();

  // Wait for initial load
  actions.push(DemoAction::Wait(Duration::from_millis(500)));

  // Step 1: Welcome - press 'j' to move down
  actions.push(DemoAction::Key(KeyCode::Char('j')));
  actions.push(DemoAction::Wait(Duration::from_millis(300)));

  // Type :next to continue
  actions.push(DemoAction::Key(KeyCode::Char(':')));
  actions.push(DemoAction::Key(KeyCode::Char('n')));
  actions.push(DemoAction::Key(KeyCode::Char('e')));
  actions.push(DemoAction::Key(KeyCode::Char('x')));
  actions.push(DemoAction::Key(KeyCode::Char('t')));
  actions.push(DemoAction::Key(KeyCode::Enter));
  actions.push(DemoAction::Wait(Duration::from_millis(500)));

  // Step 2: Visual Selection - press 'v' and move to select, then 'y' to yank
  actions.push(DemoAction::Key(KeyCode::Char('v')));
  actions.push(DemoAction::Wait(Duration::from_millis(200)));
  actions.push(DemoAction::Key(KeyCode::Char('w')));
  actions.push(DemoAction::Wait(Duration::from_millis(200)));
  actions.push(DemoAction::Key(KeyCode::Char('y')));
  actions.push(DemoAction::Wait(Duration::from_millis(300)));

  // Type :next
  actions.push(DemoAction::Key(KeyCode::Char(':')));
  actions.push(DemoAction::Key(KeyCode::Char('n')));
  actions.push(DemoAction::Key(KeyCode::Char('e')));
  actions.push(DemoAction::Key(KeyCode::Char('x')));
  actions.push(DemoAction::Key(KeyCode::Char('t')));
  actions.push(DemoAction::Key(KeyCode::Enter));
  actions.push(DemoAction::Wait(Duration::from_millis(500)));

  // Step 3: Text Objects - vip to select paragraph, then :h to highlight
  actions.push(DemoAction::Key(KeyCode::Char('v')));
  actions.push(DemoAction::Wait(Duration::from_millis(200)));
  actions.push(DemoAction::Key(KeyCode::Char('i')));
  actions.push(DemoAction::Key(KeyCode::Char('p')));
  actions.push(DemoAction::Wait(Duration::from_millis(300)));
  actions.push(DemoAction::Key(KeyCode::Char(':')));
  actions.push(DemoAction::Key(KeyCode::Char('h')));
  actions.push(DemoAction::Key(KeyCode::Enter));
  actions.push(DemoAction::Wait(Duration::from_millis(500)));

  // Type :next
  actions.push(DemoAction::Key(KeyCode::Char(':')));
  actions.push(DemoAction::Key(KeyCode::Char('n')));
  actions.push(DemoAction::Key(KeyCode::Char('e')));
  actions.push(DemoAction::Key(KeyCode::Char('x')));
  actions.push(DemoAction::Key(KeyCode::Char('t')));
  actions.push(DemoAction::Key(KeyCode::Enter));
  actions.push(DemoAction::Wait(Duration::from_millis(500)));

  // Step 4: Search - search for "special"
  actions.push(DemoAction::Key(KeyCode::Char('/')));
  actions.push(DemoAction::Wait(Duration::from_millis(200)));
  actions.push(DemoAction::Key(KeyCode::Char('s')));
  actions.push(DemoAction::Key(KeyCode::Char('p')));
  actions.push(DemoAction::Key(KeyCode::Char('e')));
  actions.push(DemoAction::Key(KeyCode::Char('c')));
  actions.push(DemoAction::Key(KeyCode::Char('i')));
  actions.push(DemoAction::Key(KeyCode::Char('a')));
  actions.push(DemoAction::Key(KeyCode::Char('l')));
  actions.push(DemoAction::Key(KeyCode::Enter));
  actions.push(DemoAction::Wait(Duration::from_millis(500)));

  // Type :next
  actions.push(DemoAction::Key(KeyCode::Char(':')));
  actions.push(DemoAction::Key(KeyCode::Char('n')));
  actions.push(DemoAction::Key(KeyCode::Char('e')));
  actions.push(DemoAction::Key(KeyCode::Char('x')));
  actions.push(DemoAction::Key(KeyCode::Char('t')));
  actions.push(DemoAction::Key(KeyCode::Enter));
  actions.push(DemoAction::Wait(Duration::from_millis(500)));

  // Step 5: Bookmarks - set bookmark 'a' with 'ma'
  actions.push(DemoAction::Key(KeyCode::Char('m')));
  actions.push(DemoAction::Wait(Duration::from_millis(200)));
  actions.push(DemoAction::Key(KeyCode::Char('a')));
  actions.push(DemoAction::Wait(Duration::from_millis(500)));

  // Type :next
  actions.push(DemoAction::Key(KeyCode::Char(':')));
  actions.push(DemoAction::Key(KeyCode::Char('n')));
  actions.push(DemoAction::Key(KeyCode::Char('e')));
  actions.push(DemoAction::Key(KeyCode::Char('x')));
  actions.push(DemoAction::Key(KeyCode::Char('t')));
  actions.push(DemoAction::Key(KeyCode::Enter));
  actions.push(DemoAction::Wait(Duration::from_millis(500)));

  // Step 6: Command Execution - run :!echo hello world
  actions.push(DemoAction::Key(KeyCode::Char(':')));
  actions.push(DemoAction::Key(KeyCode::Char('!')));
  actions.push(DemoAction::Key(KeyCode::Char('e')));
  actions.push(DemoAction::Key(KeyCode::Char('c')));
  actions.push(DemoAction::Key(KeyCode::Char('h')));
  actions.push(DemoAction::Key(KeyCode::Char('o')));
  actions.push(DemoAction::Key(KeyCode::Char(' ')));
  actions.push(DemoAction::Key(KeyCode::Char('h')));
  actions.push(DemoAction::Key(KeyCode::Char('e')));
  actions.push(DemoAction::Key(KeyCode::Char('l')));
  actions.push(DemoAction::Key(KeyCode::Char('l')));
  actions.push(DemoAction::Key(KeyCode::Char('o')));
  actions.push(DemoAction::Key(KeyCode::Char(' ')));
  actions.push(DemoAction::Key(KeyCode::Char('w')));
  actions.push(DemoAction::Key(KeyCode::Char('o')));
  actions.push(DemoAction::Key(KeyCode::Char('r')));
  actions.push(DemoAction::Key(KeyCode::Char('l')));
  actions.push(DemoAction::Key(KeyCode::Char('d')));
  actions.push(DemoAction::Key(KeyCode::Enter));
  actions.push(DemoAction::Wait(Duration::from_millis(1000))); // Wait for command execution

  // Type :next
  actions.push(DemoAction::Key(KeyCode::Char(':')));
  actions.push(DemoAction::Key(KeyCode::Char('n')));
  actions.push(DemoAction::Key(KeyCode::Char('e')));
  actions.push(DemoAction::Key(KeyCode::Char('x')));
  actions.push(DemoAction::Key(KeyCode::Char('t')));
  actions.push(DemoAction::Key(KeyCode::Enter));
  actions.push(DemoAction::Wait(Duration::from_millis(500)));

  // Final step: Press Enter to complete tutorial
  actions.push(DemoAction::Key(KeyCode::Enter));
  actions.push(DemoAction::Wait(Duration::from_millis(1000)));

  DemoScript { actions }
}
