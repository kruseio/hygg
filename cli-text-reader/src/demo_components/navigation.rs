use crate::demo_script::DemoAction;
use crossterm::event::{KeyCode, KeyModifiers};
use std::time::Duration;

use super::DemoComponent;

// ===== Navigation Components =====

pub(crate) fn basic_navigation_component() -> DemoComponent {
  use DemoAction::*;

  DemoComponent {
    id: "basic_navigation",
    name: "Basic Navigation",
    description: "Basic j/k/h/l movements",
    actions: vec![
      ShowHint(
        "navigate with vim keys\nj=down k=up h=left l=right".to_string(),
        Duration::from_millis(3000),
      ),
      Wait(Duration::from_millis(500)),
      Key(KeyCode::Char('j')),
      Wait(Duration::from_millis(300)),
      Key(KeyCode::Char('j')),
      Wait(Duration::from_millis(300)),
      Key(KeyCode::Char('k')),
      Wait(Duration::from_millis(300)),
      Key(KeyCode::Char('l')),
      Wait(Duration::from_millis(300)),
      Key(KeyCode::Char('h')),
      Wait(Duration::from_millis(500)),
    ],
  }
}

pub(crate) fn word_navigation_component() -> DemoComponent {
  use DemoAction::*;

  DemoComponent {
    id: "word_navigation",
    name: "Word Navigation",
    description: "w/b/e word movements",
    actions: vec![
      ShowHint(
        "jump between words\nw=next b=back e=end".to_string(),
        Duration::from_millis(3000),
      ),
      Wait(Duration::from_millis(500)),
      Key(KeyCode::Char('w')),
      Wait(Duration::from_millis(300)),
      Key(KeyCode::Char('w')),
      Wait(Duration::from_millis(300)),
      Key(KeyCode::Char('b')),
      Wait(Duration::from_millis(300)),
      Key(KeyCode::Char('e')),
      Wait(Duration::from_millis(500)),
    ],
  }
}

pub(crate) fn paragraph_navigation_component() -> DemoComponent {
  use DemoAction::*;

  DemoComponent {
    id: "paragraph_navigation",
    name: "Paragraph Navigation",
    description: "{ } paragraph jumps",
    actions: vec![
      ShowHint(
        "jump between paragraphs\n{ = previous } = next".to_string(),
        Duration::from_millis(3000),
      ),
      Wait(Duration::from_millis(500)),
      Key(KeyCode::Char('}')),
      Wait(Duration::from_millis(500)),
      Key(KeyCode::Char('}')),
      Wait(Duration::from_millis(500)),
      Key(KeyCode::Char('{')),
      Wait(Duration::from_millis(500)),
    ],
  }
}

pub(crate) fn search_navigation_component() -> DemoComponent {
  use DemoAction::*;

  DemoComponent {
    id: "search_navigation",
    name: "Search Navigation",
    description: "/ search and n/N navigation",
    actions: vec![
      ShowHint(
        "search for text\n/ to search, n/N to navigate".to_string(),
        Duration::from_millis(3000),
      ),
      Wait(Duration::from_millis(500)),
      Key(KeyCode::Char('/')),
      Wait(Duration::from_millis(250)),
      TypeString("reader".to_string(), Duration::from_millis(100)),
      Wait(Duration::from_millis(250)),
      Key(KeyCode::Enter),
      Wait(Duration::from_millis(500)),
      Key(KeyCode::Char('n')),
      Wait(Duration::from_millis(500)),
      Key(KeyCode::Char('N')),
      Wait(Duration::from_millis(500)),
    ],
  }
}

// ===== Additional Navigation Components =====

pub(crate) fn simple_jjj_navigation_component() -> DemoComponent {
  use DemoAction::*;

  DemoComponent {
    id: "simple_jjj_navigation",
    name: "Simple JJJ Navigation",
    description: "Just 3 j movements down",
    actions: vec![
      ShowHint(
        "navigate to the file listing\nusing j to move down".to_string(),
        Duration::from_millis(3000),
      ),
      Wait(Duration::from_millis(500)),
      Key(KeyCode::Char('j')),
      Wait(Duration::from_millis(300)),
      Key(KeyCode::Char('j')),
      Wait(Duration::from_millis(300)),
      Key(KeyCode::Char('j')),
      Wait(Duration::from_millis(500)),
    ],
  }
}

pub(crate) fn advanced_navigation_component() -> DemoComponent {
  use DemoAction::*;

  DemoComponent {
    id: "advanced_navigation",
    name: "Advanced Navigation",
    description: "gg/G/Ctrl-f/Ctrl-b movements",
    actions: vec![
      ShowHint(
        "advanced navigation\ngg=top G=bottom Ctrl-f/b=page".to_string(),
        Duration::from_millis(3500),
      ),
      Wait(Duration::from_millis(500)),
      VimMotion("gg".to_string()),
      Wait(Duration::from_millis(500)),
      Key(KeyCode::Char('G')),
      Wait(Duration::from_millis(500)),
      KeyWithModifiers(KeyCode::Char('f'), KeyModifiers::CONTROL),
      Wait(Duration::from_millis(500)),
      KeyWithModifiers(KeyCode::Char('b'), KeyModifiers::CONTROL),
      Wait(Duration::from_millis(500)),
    ],
  }
}
