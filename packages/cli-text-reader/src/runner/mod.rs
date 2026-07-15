mod config;
mod entry;
mod pdf;
mod pdf_position;

#[cfg(test)]
mod tests;

pub use crate::editor::RunOutcome;
pub use config::{
  load_ocr_enabled_config, save_ocr_enabled_config, save_tts_enabled_config,
};
pub use entry::{
  run_cli_text_reader, run_cli_text_reader_with_content,
  run_cli_text_reader_with_demo, run_cli_text_reader_with_demo_id,
  run_cli_text_reader_with_source,
};
pub use pdf::{
  run_cli_text_reader_pdf_path, run_cli_text_reader_pdf_path_with_bundled_ocr,
};
