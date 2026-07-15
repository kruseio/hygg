use crate::demo_script::DemoAction;
use crossterm::event::{KeyCode, KeyModifiers};
use std::time::Duration;

use super::DemoComponent;

// ===== Command Components =====

pub(crate) fn execute_ls_component() -> DemoComponent {
  use DemoAction::*;

  DemoComponent {
    id: "execute_ls",
    name: "Execute ls Command",
    description: "List files with :!ls",
    actions: vec![
      ShowHint("execute commands".to_string(), Duration::from_millis(3500)),
      Wait(Duration::from_millis(500)),
      Key(KeyCode::Char(':')),
      Wait(Duration::from_millis(250)),
      Key(KeyCode::Char('!')),
      Wait(Duration::from_millis(250)),
      Key(KeyCode::Char('l')),
      Wait(Duration::from_millis(250)),
      Key(KeyCode::Char('s')),
      Wait(Duration::from_millis(500)),
      Key(KeyCode::Enter),
      Wait(Duration::from_millis(1000)),
    ],
  }
}

pub(crate) fn execute_cat_component() -> DemoComponent {
  use DemoAction::*;

  DemoComponent {
    id: "execute_cat",
    name: "Execute cat Command",
    description: ":!cat filename",
    actions: vec![
      ShowHint(
        "view file contents\nwith :!cat command".to_string(),
        Duration::from_millis(3500),
      ),
      Wait(Duration::from_millis(500)),
      Key(KeyCode::Char(':')),
      Wait(Duration::from_millis(250)),
      Key(KeyCode::Char('!')),
      Wait(Duration::from_millis(250)),
      TypeString("cat README.md".to_string(), Duration::from_millis(100)),
      Wait(Duration::from_millis(500)),
      Key(KeyCode::Enter),
      Wait(Duration::from_millis(1500)),
    ],
  }
}

pub(crate) fn execute_grep_component() -> DemoComponent {
  use DemoAction::*;

  DemoComponent {
    id: "execute_grep",
    name: "Execute grep Command",
    description: ":!grep pattern file",
    actions: vec![
      ShowHint(
        "search file contents\nwith :!grep command".to_string(),
        Duration::from_millis(3500),
      ),
      Wait(Duration::from_millis(500)),
      Key(KeyCode::Char(':')),
      Wait(Duration::from_millis(250)),
      Key(KeyCode::Char('!')),
      Wait(Duration::from_millis(250)),
      TypeString("grep TODO *.md".to_string(), Duration::from_millis(100)),
      Wait(Duration::from_millis(500)),
      Key(KeyCode::Enter),
      Wait(Duration::from_millis(1500)),
    ],
  }
}

pub(crate) fn yank_and_execute_component() -> DemoComponent {
  use DemoAction::*;

  DemoComponent {
    id: "yank_and_execute",
    name: "Yank and Execute",
    description: "Yank then paste in command",
    actions: vec![
      ShowHint(
        "paste yanked text\ninto commands with Ctrl+V".to_string(),
        Duration::from_millis(3500),
      ),
      Wait(Duration::from_millis(500)),
      Key(KeyCode::Char(':')),
      Wait(Duration::from_millis(250)),
      Key(KeyCode::Char('!')),
      Wait(Duration::from_millis(250)),
      Key(KeyCode::Char('c')),
      Wait(Duration::from_millis(250)),
      Key(KeyCode::Char('a')),
      Wait(Duration::from_millis(250)),
      Key(KeyCode::Char('t')),
      Wait(Duration::from_millis(250)),
      Key(KeyCode::Char(' ')),
      Wait(Duration::from_millis(250)),
      KeyWithModifiers(KeyCode::Char('v'), KeyModifiers::CONTROL),
      Wait(Duration::from_millis(500)),
      Key(KeyCode::Enter),
      Wait(Duration::from_millis(2000)),
    ],
  }
}

// ===== Additional Command Components =====

pub(crate) fn execute_cat_with_paste_component() -> DemoComponent {
  use DemoAction::*;

  DemoComponent {
    id: "execute_cat_with_paste",
    name: "Execute cat with Paste",
    description: ":!cat with Ctrl+V to paste yanked text",
    actions: vec![
      ShowHint(
        "paste into next command".to_string(),
        Duration::from_millis(3500),
      ),
      Wait(Duration::from_millis(500)),
      Key(KeyCode::Char(':')),
      Wait(Duration::from_millis(250)),
      Key(KeyCode::Char('!')),
      Wait(Duration::from_millis(250)),
      Key(KeyCode::Char('c')),
      Wait(Duration::from_millis(100)),
      Key(KeyCode::Char('a')),
      Wait(Duration::from_millis(100)),
      Key(KeyCode::Char('t')),
      Wait(Duration::from_millis(100)),
      Key(KeyCode::Char(' ')),
      Wait(Duration::from_millis(250)),
      KeyWithModifiers(KeyCode::Char('v'), KeyModifiers::CONTROL),
      Wait(Duration::from_millis(500)),
      Key(KeyCode::Enter),
      Wait(Duration::from_millis(2000)),
    ],
  }
}

pub(crate) fn search_cargo_component() -> DemoComponent {
  use DemoAction::*;

  DemoComponent {
    id: "search_cargo",
    name: "Search in command output",
    description: "Search in command output",
    actions: vec![
      ShowHint(
        "search in command output".to_string(),
        Duration::from_millis(3000),
      ),
      Wait(Duration::from_millis(500)),
      Key(KeyCode::Char('/')),
      Wait(Duration::from_millis(250)),
      TypeString("toml".to_string(), Duration::from_millis(100)),
      Wait(Duration::from_millis(500)),
      Key(KeyCode::Enter),
      Wait(Duration::from_millis(1000)),
    ],
  }
}

pub(crate) fn split_view_component() -> DemoComponent {
  use DemoAction::*;

  DemoComponent {
    id: "split_view",
    name: "Split View",
    description: "Demo split view after command execution",
    actions: vec![
      ShowHint(
        "command output appears\nin a split view".to_string(),
        Duration::from_millis(3000),
      ),
      Wait(Duration::from_millis(500)),
      // The split view appears automatically after command execution
      // Just demonstrate navigation in split view
      Key(KeyCode::Char('j')),
      Wait(Duration::from_millis(300)),
      Key(KeyCode::Char('k')),
      Wait(Duration::from_millis(500)),
    ],
  }
}

// ===== UI Components =====

pub(crate) fn intro_message_component() -> DemoComponent {
  use DemoAction::*;

  DemoComponent {
    id: "intro_message",
    name: "Intro Message",
    description: "Opening message",
    actions: vec![Wait(Duration::from_millis(2000))],
  }
}

pub(crate) fn final_message_component() -> DemoComponent {
  use DemoAction::*;

  DemoComponent {
    id: "final_message",
    name: "Final Message",
    description: "Closing with github link",
    actions: vec![
      ShowHint(
        "hygg - simplifying the way you read\n\ngithub.com/kruseio/hygg"
          .to_string(),
        Duration::from_millis(5000),
      ),
      Wait(Duration::from_millis(4000)),
    ],
  }
}

pub(crate) fn final_message_short_component() -> DemoComponent {
  use DemoAction::*;

  DemoComponent {
    id: "final_message_short",
    name: "Final Message Short",
    description: "Short closing message",
    actions: vec![
      ShowHint(
        "hygg - simplifying the way you read\n\ngithub.com/kruseio/hygg"
          .to_string(),
        Duration::from_millis(3000),
      ),
      Wait(Duration::from_millis(2000)),
    ],
  }
}
