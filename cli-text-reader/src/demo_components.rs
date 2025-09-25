use crate::demo_script::DemoAction;
use crossterm::event::{KeyCode, KeyModifiers};
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct DemoComponent {
  pub id: &'static str,          // Unique identifier
  pub name: &'static str,        // Human-readable name
  pub description: &'static str, // Brief description
  pub actions: Vec<DemoAction>,  // Sequence of actions
}

// Get a component by its ID
pub fn get_component(id: &str) -> Option<DemoComponent> {
  match id {
    // Navigation Components
    "basic_navigation" => Some(basic_navigation_component()),
    "word_navigation" => Some(word_navigation_component()),
    "paragraph_navigation" => Some(paragraph_navigation_component()),
    "search_navigation" => Some(search_navigation_component()),

    // Selection Components
    "select_word" => Some(select_word_component()),
    "select_paragraph" => Some(select_paragraph_component()),
    "select_line" => Some(select_line_component()),
    "visual_char_mode" => Some(visual_char_mode_component()),

    // Action Components
    "yank_line" => Some(yank_line_component()),
    "yank_selection" => Some(yank_selection_component()),
    "highlight_selection" => Some(highlight_selection_component()),
    "clear_highlights" => Some(clear_highlights_component()),

    // Command Components
    "execute_ls" => Some(execute_ls_component()),
    "execute_cat" => Some(execute_cat_component()),
    "execute_grep" => Some(execute_grep_component()),
    "yank_and_execute" => Some(yank_and_execute_component()),
    "execute_cat_with_paste" => Some(execute_cat_with_paste_component()),

    // Additional Navigation Components
    "simple_jjj_navigation" => Some(simple_jjj_navigation_component()),
    "search_cargo" => Some(search_cargo_component()),
    "advanced_navigation" => Some(advanced_navigation_component()),
    "multi_select" => Some(multi_select_component()),
    "split_view" => Some(split_view_component()),

    // UI Components
    "intro_message" => Some(intro_message_component()),
    "final_message" => Some(final_message_component()),
    "final_message_short" => Some(final_message_short_component()),

    _ => None,
  }
}

// List all available components
pub fn list_all_components() -> Vec<DemoComponent> {
  vec![
    // Navigation Components
    basic_navigation_component(),
    word_navigation_component(),
    paragraph_navigation_component(),
    search_navigation_component(),
    // Selection Components
    select_word_component(),
    select_paragraph_component(),
    select_line_component(),
    visual_char_mode_component(),
    // Action Components
    yank_line_component(),
    yank_selection_component(),
    highlight_selection_component(),
    clear_highlights_component(),
    // Command Components
    execute_ls_component(),
    execute_cat_component(),
    execute_grep_component(),
    yank_and_execute_component(),
    execute_cat_with_paste_component(),
    // Additional Navigation Components
    simple_jjj_navigation_component(),
    search_cargo_component(),
    advanced_navigation_component(),
    multi_select_component(),
    split_view_component(),
    // UI Components
    intro_message_component(),
    final_message_component(),
    final_message_short_component(),
  ]
}

// ===== Navigation Components =====

fn basic_navigation_component() -> DemoComponent {
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

fn word_navigation_component() -> DemoComponent {
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

fn paragraph_navigation_component() -> DemoComponent {
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

fn search_navigation_component() -> DemoComponent {
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

// ===== Selection Components =====

fn select_word_component() -> DemoComponent {
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

fn select_paragraph_component() -> DemoComponent {
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

fn select_line_component() -> DemoComponent {
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

fn visual_char_mode_component() -> DemoComponent {
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

// ===== Action Components =====

fn yank_line_component() -> DemoComponent {
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

fn yank_selection_component() -> DemoComponent {
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

fn highlight_selection_component() -> DemoComponent {
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

fn clear_highlights_component() -> DemoComponent {
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

// ===== Command Components =====

fn execute_ls_component() -> DemoComponent {
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

fn execute_cat_component() -> DemoComponent {
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

fn execute_grep_component() -> DemoComponent {
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

fn yank_and_execute_component() -> DemoComponent {
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

// ===== UI Components =====

fn intro_message_component() -> DemoComponent {
  use DemoAction::*;

  DemoComponent {
    id: "intro_message",
    name: "Intro Message",
    description: "Opening message",
    actions: vec![Wait(Duration::from_millis(2000))],
  }
}

fn final_message_component() -> DemoComponent {
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

fn final_message_short_component() -> DemoComponent {
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

// ===== Additional Navigation Components =====

fn simple_jjj_navigation_component() -> DemoComponent {
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

fn search_cargo_component() -> DemoComponent {
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

// ===== Additional Command Components =====

fn execute_cat_with_paste_component() -> DemoComponent {
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

fn advanced_navigation_component() -> DemoComponent {
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

fn multi_select_component() -> DemoComponent {
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

fn split_view_component() -> DemoComponent {
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_get_component_valid() {
    let component = get_component("basic_navigation");
    assert!(component.is_some());
    let component = component.unwrap();
    assert_eq!(component.id, "basic_navigation");
    assert_eq!(component.name, "Basic Navigation");
  }

  #[test]
  fn test_get_component_invalid() {
    let component = get_component("nonexistent_component");
    assert!(component.is_none());
  }

  #[test]
  fn test_list_all_components_count() {
    let components = list_all_components();
    // We have 22 components total
    assert!(components.len() >= 22);
  }

  #[test]
  fn test_component_actions_not_empty() {
    let component = get_component("select_paragraph").unwrap();
    assert!(!component.actions.is_empty());
  }

  #[test]
  fn test_new_components_exist() {
    assert!(get_component("simple_jjj_navigation").is_some());
    assert!(get_component("search_cargo").is_some());
    assert!(get_component("execute_cat_with_paste").is_some());
    assert!(get_component("advanced_navigation").is_some());
    assert!(get_component("multi_select").is_some());
    assert!(get_component("split_view").is_some());
  }
}
