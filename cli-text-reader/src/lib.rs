mod bookmarks;
mod config;
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
mod interactive_tutorial;
mod interactive_tutorial_buffer;
mod interactive_tutorial_steps;
mod interactive_tutorial_tests;
mod interactive_tutorial_utils;
mod progress;
mod tutorial;
mod utils;

mod runner;

pub use runner::{
  load_ocr_enabled_config, run_cli_text_reader, run_cli_text_reader_pdf_path,
  run_cli_text_reader_pdf_path_with_bundled_ocr,
  run_cli_text_reader_with_content, run_cli_text_reader_with_demo,
  run_cli_text_reader_with_demo_id, save_ocr_enabled_config,
};
