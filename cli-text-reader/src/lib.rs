mod bookmarks;
pub mod config;
mod core_state;
mod core_types;
mod debug;
pub mod demo_components;
mod demo_content;
pub mod demo_registry;
pub mod demo_script;
mod demo_tutorial_test;
mod editor;
mod help;
mod highlights;
mod highlights_core;
mod highlights_persistence;
mod home;
mod interactive_tutorial;
mod interactive_tutorial_buffer;
mod interactive_tutorial_steps;
mod interactive_tutorial_tests;
mod interactive_tutorial_utils;
mod library;
mod notes;
mod progress;
mod reading_stats;
pub mod sync;
mod tutorial;
mod utils;
mod word_anchor;

mod runner;

pub use config::config_env_path;
pub use home::{run_home, should_show_home};
pub use runner::{
  RunOutcome, load_ocr_enabled_config, run_cli_text_reader,
  run_cli_text_reader_pdf_path, run_cli_text_reader_pdf_path_with_bundled_ocr,
  run_cli_text_reader_with_content, run_cli_text_reader_with_demo,
  run_cli_text_reader_with_demo_id, run_cli_text_reader_with_source,
  save_ocr_enabled_config, save_tts_enabled_config,
};
