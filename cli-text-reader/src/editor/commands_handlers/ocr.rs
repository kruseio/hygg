use super::super::core::{Editor, EditorMode};
use crate::config::{AppConfig, save_config};

fn save_ocr_config_for_command(
  config: &AppConfig,
) -> Result<(), Box<dyn std::error::Error>> {
  #[cfg(test)]
  if std::env::var_os("HYGG_TEST_WRITE_CONFIG").is_none() {
    return Ok(());
  }

  save_config(config)
}

impl Editor {
  pub fn handle_ocr_command(
    &mut self,
    enable: bool,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    self.ocr_enabled = enable;
    let config = AppConfig {
      enable_tutorial: None,
      enable_line_highlighter: None,
      show_cursor: None,
      show_progress: None,
      pdf_ocr: Some(enable),
      tutorial_shown: None,
    };
    if let Err(e) = save_ocr_config_for_command(&config) {
      self.debug_log_error(&format!("Failed to save OCR config: {e}"));
    }

    if enable {
      self.start_pdf_ocr_loader();
    } else {
      self.stop_pdf_ocr_loader();
    }
    self.set_active_mode(EditorMode::Normal);
    self.editor_state.command_buffer.clear();
    self.editor_state.command_cursor_pos = 0;
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.command_buffer.clear();
      buffer.command_cursor_pos = 0;
    }
    self.mark_dirty();
    Ok(false)
  }
}
