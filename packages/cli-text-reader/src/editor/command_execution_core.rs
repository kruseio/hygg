use super::core::{Editor, ViewMode};
use crate::editor::command_execution_security::{
  execute_secure_command_with_timeout, parse_secure_command,
};
use std::time::Duration;

impl Editor {
  // Execute a shell command in tutorial mode (with proper split handling)
  pub fn execute_shell_command_in_tutorial(
    &mut self,
    cmd: &str,
  ) -> Result<(), Box<dyn std::error::Error>> {
    self.debug_log(&format!(
      "=== execute_shell_command_in_tutorial: '{cmd}' ==="
    ));

    // Track command execution for tutorial
    let full_cmd = format!("!{cmd}");
    self.debug_log(&format!("Setting last_executed_command to '{full_cmd}'"));
    self.last_executed_command = Some(full_cmd);

    // Check if this completes the tutorial step (but don't auto-advance)
    if !self.tutorial_step_completed && self.check_tutorial_completion() {
      self.debug_log("Tutorial step completed!");
      self.tutorial_step_completed = true;
      // Update display to show :next hint
      self.update_tutorial_step();
    } else {
      self.debug_log(&format!(
        "Tutorial step not completed. last_executed_command={:?}",
        self.last_executed_command
      ));
    }

    // Execute the command normally with a split - same as non-tutorial mode
    // This allows users to see and copy the output
    self.execute_shell_command(cmd)?;

    self.debug_log(&format!(
      "Tutorial mode: Command '{cmd}' executed with split"
    ));

    Ok(())
  }

  // Execute a shell command and display output in a new buffer
  pub fn execute_shell_command(
    &mut self,
    cmd: &str,
  ) -> Result<(), Box<dyn std::error::Error>> {
    self.debug_log(&format!("=== execute_shell_command: '{cmd}' ==="));
    self.debug_log("  Current state:");
    self.debug_log(&format!(
      "    Buffers: {}, active: {}",
      self.buffers.len(),
      self.active_buffer
    ));
    self.debug_log(&format!("    View mode: {:?}", self.view_mode));
    self.debug_log(&format!("    Editor mode: {:?}", self.editor_state.mode));
    self.debug_log(&format!(
      "    Cursor: ({}, {}), offset: {}",
      self.cursor_x, self.cursor_y, self.offset
    ));

    // Parse and validate the command securely
    let parsed_cmd = match parse_secure_command(cmd) {
      Ok(cmd) => cmd,
      Err(e) => {
        let error_lines = vec![
          format!("$ {}", cmd),
          format!("Security Error: {}", e),
          "Only whitelisted commands are allowed.".to_string(),
          "File viewing: cat, less, more, head, tail, file, stat, wc".to_string(),
          "Navigation: ls, pwd, find, locate, which, whereis".to_string(),
          "Text processing: grep, awk, sed, sort, uniq, cut, tr".to_string(),
          "System info: date, uptime, whoami, id, uname, hostname, df, free, ps".to_string(),
          "Use :help for more information about available commands.".to_string(),
        ];
        self.create_horizontal_split(cmd, error_lines);
        return Ok(());
      }
    };

    // Execute the validated command
    let output = match execute_secure_command_with_timeout(
      parsed_cmd,
      Duration::from_secs(30),
    ) {
      Ok(output) => output,
      Err(e) => {
        // Create error buffer
        let error_lines = vec![format!("$ {}", cmd), format!("Error: {}", e)];
        self.create_horizontal_split(cmd, error_lines);
        return Ok(());
      }
    };

    // Prepare output lines
    let mut lines = Vec::new();
    lines.push(format!("$ {cmd}"));

    // Add stdout lines
    if !output.stdout.is_empty() {
      for line in output.stdout.lines() {
        lines.push(line.to_string());
      }
    }

    // Add stderr lines if any
    if !output.stderr.is_empty() {
      if !output.stdout.is_empty() {
        lines.push("--- stderr ---".to_string());
      }
      for line in output.stderr.lines() {
        lines.push(line.to_string());
      }
    }

    // If no output at all
    if output.stdout.is_empty() && output.stderr.is_empty() {
      lines.push("(no output)".to_string());
    }

    // Add exit status if non-zero
    if !output.status.success() {
      lines.push(format!(
        "--- exit status: {} ---",
        output.status.code().unwrap_or(-1)
      ));
    }

    // Create horizontal split with command output
    self.create_horizontal_split(cmd, lines);

    self.debug_log("=== execute_shell_command complete ===");
    Ok(())
  }
}
