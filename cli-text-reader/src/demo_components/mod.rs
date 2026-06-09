use crate::demo_script::DemoAction;

mod actions;
mod commands;
mod navigation;
mod selection;

use actions::*;
use commands::*;
use navigation::*;
use selection::*;

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
