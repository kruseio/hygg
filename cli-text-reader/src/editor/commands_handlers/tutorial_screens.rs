use super::super::core::{Editor, EditorMode};
use crate::config::{AppConfig, save_config};

impl Editor {
  // Handle :credits/:author command - show credits screen
  pub fn handle_credits_command(
    &mut self,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    // Get the last step (credits) from tutorial
    let steps = crate::interactive_tutorial::get_interactive_tutorial_steps();
    if let Some(credits_step) = steps.last() {
      // Create a simple overlay with just the credits content
      let mut lines = Vec::new();

      // Header
      lines.push(format!("━━━ {} ━━━", credits_step.title));
      lines.push("".to_string());

      // Filter out any lines that mention ":next"
      for line in &credits_step.instructions {
        if !line.contains(":next") {
          lines.push(line.clone());
        }
      }
      lines.push("".to_string());

      // Practice text (additional info)
      lines.extend(credits_step.practice_text.clone());
      lines.push("".to_string());

      // Footer
      lines.push("Type :q to close this screen".to_string());

      self.create_overlay("credits", lines);
    }

    self.set_active_mode(EditorMode::Normal);
    self.editor_state.command_buffer.clear();
    self.editor_state.command_cursor_pos = 0;
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.command_buffer.clear();
      buffer.command_cursor_pos = 0;
    }
    Ok(false)
  }

  // Handle :about command - show welcome screen
  pub fn handle_about_command(
    &mut self,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    // Get the first step (welcome) from tutorial
    let steps = crate::interactive_tutorial::get_interactive_tutorial_steps();
    if let Some(welcome_step) = steps.first() {
      // Create a simple overlay with just the welcome content
      let mut lines = Vec::new();

      // Header
      lines.push(format!("━━━ {} ━━━", welcome_step.title));
      lines.push("".to_string());

      // Filter out any lines that mention ":next"
      for line in &welcome_step.instructions {
        if !line.contains(":next") {
          lines.push(line.clone());
        }
      }
      lines.push("".to_string());

      // Practice text (logo)
      lines.extend(welcome_step.practice_text.clone());
      lines.push("".to_string());

      // Footer
      lines.push("Type :q to close this screen".to_string());

      self.create_overlay("about", lines);
    }

    self.set_active_mode(EditorMode::Normal);
    self.editor_state.command_buffer.clear();
    self.editor_state.command_cursor_pos = 0;
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.command_buffer.clear();
      buffer.command_cursor_pos = 0;
    }
    Ok(false)
  }

  // Handle :notutorial command - permanently disable tutorial
  pub fn handle_notutorial_command(
    &mut self,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    self.debug_log("Handling :notutorial command");

    // Disable tutorial in config
    let config = AppConfig {
      enable_tutorial: Some(false),
      enable_line_highlighter: None, // Keep existing value
      show_cursor: None,             // Keep existing value
      show_progress: None,           // Keep existing value
      show_page_numbers: None,       // Keep existing value
      pdf_ocr: None,                 // Keep existing value
      tutorial_shown: None,          // Keep existing value
    };

    if let Err(e) = save_config(&config) {
      self.debug_log_error(&format!("Failed to save config: {e}"));
    }

    // If we're currently in tutorial, exit it
    if self.tutorial_active {
      self.debug_log("Exiting tutorial due to :notutorial command");
      self.complete_tutorial_interactive();
    }

    // Show confirmation message
    let message = vec![
      "".to_string(),
      "Tutorial disabled permanently.".to_string(),
      "".to_string(),
      "Use :tutorial on to re-enable.".to_string(),
      "".to_string(),
      "Press :q to close this message.".to_string(),
    ];
    self.create_overlay("notification", message);

    self.set_active_mode(EditorMode::Normal);
    self.editor_state.command_buffer.clear();
    self.editor_state.command_cursor_pos = 0;
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.command_buffer.clear();
      buffer.command_cursor_pos = 0;
    }
    Ok(false)
  }

  // Handle :tutorial on/off command - toggle tutorial
  pub fn handle_tutorial_toggle_command(
    &mut self,
    enable: bool,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    self.debug_log(&format!(
      "Handling :tutorial {} command",
      if enable { "on" } else { "off" }
    ));

    // Update tutorial setting in config
    let config = AppConfig {
      enable_tutorial: Some(enable),
      enable_line_highlighter: None, // Keep existing value
      show_cursor: None,             // Keep existing value
      show_progress: None,           // Keep existing value
      show_page_numbers: None,       // Keep existing value
      pdf_ocr: None,                 // Keep existing value
      tutorial_shown: None,          // Keep existing value
    };

    if let Err(e) = save_config(&config) {
      self.debug_log_error(&format!("Failed to save config: {e}"));
    }

    // If disabling and currently in tutorial, exit it
    if !enable && self.tutorial_active {
      self.debug_log("Exiting tutorial due to :tutorial off command");
      self.complete_tutorial_interactive();
    }

    // Show confirmation message
    let message = vec![
      "".to_string(),
      format!(
        "Tutorial {} for next launch.",
        if enable { "enabled" } else { "disabled" }
      ),
      "".to_string(),
      if enable {
        "The tutorial will show on next startup if not already completed."
          .to_string()
      } else {
        "Use :tutorial on to re-enable.".to_string()
      },
      "".to_string(),
      "Press :q to close this message.".to_string(),
    ];
    self.create_overlay("notification", message);

    self.set_active_mode(EditorMode::Normal);
    self.editor_state.command_buffer.clear();
    self.editor_state.command_cursor_pos = 0;
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.command_buffer.clear();
      buffer.command_cursor_pos = 0;
    }
    Ok(false)
  }
}
