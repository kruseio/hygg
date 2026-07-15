use crate::demo_script::DemoAction;
use crossterm::event::KeyCode;
use std::time::Duration;

use super::DemoComponent;

// ===== Action Components =====

pub(crate) fn yank_line_component() -> DemoComponent {
  use DemoAction::*;

  DemoComponent {
    id: "yank_line",
    name: "Yank Line",
    description: "yy to yank line",
    actions: vec![
      ShowHint(
        "copy from command output".to_string(),
        Duration::from_millis(3000),
      ),
      Wait(Duration::from_millis(500)),
      Key(KeyCode::Char('y')),
      Wait(Duration::from_millis(250)),
      Key(KeyCode::Char('y')),
      Wait(Duration::from_millis(1000)),
    ],
  }
}

pub(crate) fn yank_selection_component() -> DemoComponent {
  use DemoAction::*;

  DemoComponent {
    id: "yank_selection",
    name: "Yank Selection",
    description: "y to yank selection",
    actions: vec![
      ShowHint(
        "copy selected text\nwith y command".to_string(),
        Duration::from_millis(3000),
      ),
      Wait(Duration::from_millis(500)),
      Key(KeyCode::Char('y')),
      Wait(Duration::from_millis(1000)),
    ],
  }
}

pub(crate) fn highlight_selection_component() -> DemoComponent {
  use DemoAction::*;

  DemoComponent {
    id: "highlight_selection",
    name: "Highlight Selection",
    description: ":h to highlight",
    actions: vec![
      ShowHint(
        "highlight selected text".to_string(),
        Duration::from_millis(3500),
      ),
      Wait(Duration::from_millis(500)),
      Key(KeyCode::Char(':')),
      Wait(Duration::from_millis(250)),
      Key(KeyCode::Char('h')),
      Wait(Duration::from_millis(500)),
      Key(KeyCode::Enter),
      Wait(Duration::from_millis(1500)),
    ],
  }
}

pub(crate) fn clear_highlights_component() -> DemoComponent {
  use DemoAction::*;

  DemoComponent {
    id: "clear_highlights",
    name: "Clear Highlights",
    description: ":ch to clear highlights",
    actions: vec![
      ShowHint(
        "clear all highlights\nwith :ch command".to_string(),
        Duration::from_millis(3000),
      ),
      Wait(Duration::from_millis(500)),
      Key(KeyCode::Char(':')),
      Wait(Duration::from_millis(250)),
      Key(KeyCode::Char('c')),
      Wait(Duration::from_millis(250)),
      Key(KeyCode::Char('h')),
      Wait(Duration::from_millis(500)),
      Key(KeyCode::Enter),
      Wait(Duration::from_millis(1000)),
    ],
  }
}
