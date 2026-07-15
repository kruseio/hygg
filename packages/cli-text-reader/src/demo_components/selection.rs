use crate::demo_script::DemoAction;
use crossterm::event::KeyCode;
use std::time::Duration;

use super::DemoComponent;

// ===== Selection Components =====

pub(crate) fn select_word_component() -> DemoComponent {
  use DemoAction::*;

  DemoComponent {
    id: "select_word",
    name: "Select Word",
    description: "viw to select word",
    actions: vec![
      ShowHint(
        "select entire words\nwith text objects".to_string(),
        Duration::from_millis(3000),
      ),
      Wait(Duration::from_millis(500)),
      VimMotion("viw".to_string()),
      Wait(Duration::from_millis(1000)),
      Key(KeyCode::Esc),
      Wait(Duration::from_millis(500)),
    ],
  }
}

pub(crate) fn select_paragraph_component() -> DemoComponent {
  use DemoAction::*;

  DemoComponent {
    id: "select_paragraph",
    name: "Select Paragraph",
    description: "vip to select paragraph",
    actions: vec![
      ShowHint(
        "select entire paragraphs".to_string(),
        Duration::from_millis(3500),
      ),
      Wait(Duration::from_millis(500)),
      VimMotion("vip".to_string()),
      Wait(Duration::from_millis(2000)),
    ],
  }
}

pub(crate) fn select_line_component() -> DemoComponent {
  use DemoAction::*;

  DemoComponent {
    id: "select_line",
    name: "Select Line",
    description: "V line selection",
    actions: vec![
      ShowHint(
        "select entire lines\nwith visual line mode".to_string(),
        Duration::from_millis(3000),
      ),
      Wait(Duration::from_millis(500)),
      Key(KeyCode::Char('V')),
      Wait(Duration::from_millis(500)),
      Key(KeyCode::Char('j')),
      Wait(Duration::from_millis(500)),
      Key(KeyCode::Char('j')),
      Wait(Duration::from_millis(1000)),
    ],
  }
}

pub(crate) fn visual_char_mode_component() -> DemoComponent {
  use DemoAction::*;

  DemoComponent {
    id: "visual_char_mode",
    name: "Visual Character Mode",
    description: "v + character selection",
    actions: vec![
      ShowHint(
        "precise character selection\nwith visual mode".to_string(),
        Duration::from_millis(3000),
      ),
      Wait(Duration::from_millis(500)),
      Key(KeyCode::Char('v')),
      Wait(Duration::from_millis(500)),
      Key(KeyCode::Char('w')),
      Wait(Duration::from_millis(300)),
      Key(KeyCode::Char('w')),
      Wait(Duration::from_millis(300)),
      Key(KeyCode::Char('e')),
      Wait(Duration::from_millis(1000)),
    ],
  }
}

pub(crate) fn multi_select_component() -> DemoComponent {
  use DemoAction::*;

  DemoComponent {
    id: "multi_select",
    name: "Multi Select",
    description: "Multiple visual selections demo",
    actions: vec![
      ShowHint(
        "select multiple sections\nwith visual mode".to_string(),
        Duration::from_millis(3000),
      ),
      Wait(Duration::from_millis(500)),
      Key(KeyCode::Char('v')),
      Wait(Duration::from_millis(300)),
      Key(KeyCode::Char('}')),
      Wait(Duration::from_millis(500)),
      Key(KeyCode::Char('}')),
      Wait(Duration::from_millis(500)),
      Key(KeyCode::Esc),
      Wait(Duration::from_millis(500)),
    ],
  }
}
