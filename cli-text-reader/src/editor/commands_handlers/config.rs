use super::super::core::Editor;
use crate::config::{AppConfig, save_config};

impl Editor {
  // Save current config settings to file
  pub fn save_current_config(&self) {
    let config = AppConfig {
      enable_tutorial: None, // Keep existing value
      enable_line_highlighter: Some(self.show_highlighter),
      show_cursor: Some(self.show_cursor),
      show_progress: Some(self.show_progress),
      pdf_ocr: None,        // Keep existing value
      tts_enabled: None,    // Keep existing value
      tutorial_shown: None, // Keep existing value
    };

    if let Err(e) = save_config(&config) {
      self.debug_log_error(&format!("Failed to save config: {e}"));
    }
  }
}
