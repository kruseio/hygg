// Editor module - main exports

mod buffer;
mod buffer_overlay;
mod buffer_split;
mod buffer_state;
mod char_navigation;
mod command_execution;
mod command_execution_core;
mod command_execution_security;
mod command_mode;
pub(crate) mod command_registry;
#[cfg(test)]
mod command_registry_tests;
mod command_translation;
mod commands;
mod commands_handlers;
mod commands_search;
mod core;
mod cursor;
pub mod demo_content;
mod demo_executor;
mod demo_renderer;
mod display;
mod display_init;
mod display_loop;
mod display_split;
mod event_handler;
mod highlighting;
mod highlighting_persistent;
mod highlighting_selection;
mod line_navigation;
mod movement;
mod navigation;
mod normal_control;
mod normal_mode;
mod normal_navigation;
mod normal_navigation_basic;
mod normal_navigation_find;
mod normal_navigation_jumps;
mod normal_search_visual;
mod page_navigation;
mod screen_position;
mod search_mode;
mod selection;
mod selection_basic;
mod selection_text;
mod selection_words;
pub(crate) mod speech;
mod status_line;
pub(crate) mod streaming;
pub(crate) mod streaming_loader;
mod text_objects;
mod text_objects_delimiters;
mod text_objects_paragraphs;
mod text_objects_quotes;
mod toggle_highlight;
mod tutorial_display;
mod tutorial_interactive;
mod tutorial_interactive_completion;
mod utils;
mod visual_mode;
mod visual_mode_control;
mod visual_mode_find;
mod visual_mode_movement;
mod visual_mode_objects;
mod word_navigation;
mod yank;

// Re-export main structures and functions
pub use commands::handle_command;
pub use core::{Editor, EditorMode, EditorState, RunOutcome};

// Tests
#[cfg(test)]
mod toggle_highlight_test;
