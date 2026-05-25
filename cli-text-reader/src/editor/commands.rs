use crossterm::terminal;
use std::io;

use super::command_registry::{
  RegisteredCommand, TutorialCommand, classify_command,
};
use super::core::{Editor, ViewMode};

impl Editor {
  pub fn execute_command(
    &mut self,
    _stdout: &mut io::Stdout,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    let cmd = self.get_active_command_buffer().trim().to_string();
    self.debug_log_event("command", "execute_command", &format!("cmd='{cmd}'"));

    // Track command for tutorial will be done in specific command handlers
    self.debug_log_state(
      "command",
      "buffers_count",
      &self.buffers.len().to_string(),
    );
    self.debug_log_state(
      "command",
      "active_buffer",
      &self.active_buffer.to_string(),
    );
    self.debug_log_state(
      "command",
      "view_mode",
      &format!("{:?}", self.view_mode),
    );

    let registered_command = classify_command(&cmd);

    // Handle :q, :q!, :quit, :exit commands
    if registered_command == RegisteredCommand::Quit {
      // Check if we're in horizontal split view
      if self.view_mode == ViewMode::HorizontalSplit {
        // In split view, :q closes the split from either pane
        self.debug_log_event(
          "command",
          "quit_split",
          &format!(
            "closing horizontal split from buffer {}",
            self.active_buffer
          ),
        );

        // Check if we're in tutorial mode - if so, return to tutorial overlay
        if self.tutorial_active {
          self.close_split();
          // Restore tutorial overlay
          self.update_tutorial_step();
        } else {
          self.close_split();
        }

        self.set_active_mode(super::core::EditorMode::Normal);
        self.editor_state.command_buffer.clear();
        self.editor_state.command_cursor_pos = 0;
        if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
          buffer.command_buffer.clear();
          buffer.command_cursor_pos = 0;
        }
        return Ok(false);
      } else if self.can_close_buffer() {
        // In overlay view, :q closes the overlay
        self.debug_log_event(
          "command",
          "quit_overlay",
          "closing overlay buffer",
        );

        // Check if we're closing the tutorial overlay
        if self.tutorial_active && self.active_buffer == 1 {
          self.debug_log_event(
            "command",
            "quit_tutorial",
            "properly completing tutorial on :q",
          );
          self.complete_tutorial_interactive();
        } else {
          self.close_overlay();
        }

        self.set_active_mode(super::core::EditorMode::Normal);
        self.editor_state.command_buffer.clear();
        self.editor_state.command_cursor_pos = 0;
        if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
          buffer.command_buffer.clear();
          buffer.command_cursor_pos = 0;
        }
        return Ok(false);
      } else {
        // In main buffer, :q exits the editor
        self.debug_log_event(
          "command",
          "quit_editor",
          "exiting from main buffer",
        );
        return Ok(true);
      }
    }

    // Handle command execution
    if let RegisteredCommand::Shell(shell_cmd) = &registered_command {
      // Execute shell command
      self.debug_log_event(
        "command",
        "shell_command",
        &format!("cmd='{}', from_buffer={}", shell_cmd, self.active_buffer),
      );
      self.debug_log_state(
        "command",
        "mode_before_exec",
        &format!("{:?}", self.editor_state.mode),
      );

      // Check if we're in tutorial mode - if so, handle shell commands
      // differently
      if self.tutorial_active {
        // For tutorial, show command output in overlay instead of split
        self.execute_shell_command_in_tutorial(&shell_cmd)?;
      } else {
        self.execute_shell_command(&shell_cmd)?;
      }

      self.debug_log(&format!(
        "After execute_shell_command - buffers: {}, active: {}, mode: {:?}",
        self.buffers.len(),
        self.active_buffer,
        self.view_mode
      ));
      self
        .debug_log(&format!("  Lines in active buffer: {}", self.lines.len()));

      // Ensure cursor is within bounds after command execution
      let viewport_height = self.height.saturating_sub(1);
      if self.cursor_y >= viewport_height {
        let old_y = self.cursor_y;
        self.cursor_y = viewport_height.saturating_sub(1);
        self.debug_log(&format!(
          "Adjusted cursor_y from {} to {} (viewport_height={})",
          old_y, self.cursor_y, viewport_height
        ));
      }

      self.set_active_mode(super::core::EditorMode::Normal);
      self.editor_state.command_buffer.clear();
      self.editor_state.command_cursor_pos = 0;
      if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
        buffer.command_buffer.clear();
        buffer.command_cursor_pos = 0;
      }
      self.debug_log("Command execution complete, mode set to Normal");
      return Ok(false);
    }

    match registered_command {
      RegisteredCommand::Progress => self.handle_progress_command(),
      RegisteredCommand::Cursor => self.handle_cursor_command(),
      RegisteredCommand::Help => self.handle_help_command(),
      RegisteredCommand::NoTutorial => self.handle_notutorial_command(),
      RegisteredCommand::Tutorial(TutorialCommand::Default) => {
        self.handle_tutorial_command()
      }
      RegisteredCommand::Tutorial(TutorialCommand::Enabled(enabled)) => {
        self.handle_tutorial_toggle_command(enabled)
      }
      RegisteredCommand::Tutorial(TutorialCommand::Step(step)) => {
        self.handle_tutorial_command_with_step(step)
      }
      RegisteredCommand::Next => self.handle_next_command(),
      RegisteredCommand::Back => self.handle_back_command(),
      RegisteredCommand::Highlight => self.handle_highlight_command(),
      RegisteredCommand::NoHighlight => self.handle_nohl_command(),
      RegisteredCommand::Credits => self.handle_credits_command(),
      RegisteredCommand::About => self.handle_about_command(),
      RegisteredCommand::Ocr(enable) => self.handle_ocr_command(enable),
      RegisteredCommand::ToggleHighlighter => {
        self.show_highlighter = !self.show_highlighter;
        self.save_current_config();
        Ok(false)
      }
      RegisteredCommand::Unknown => {
        let result = handle_command(&cmd, &mut self.show_highlighter);
        if cmd == "z" {
          self.save_current_config();
        }
        Ok(result)
      }
      RegisteredCommand::Quit | RegisteredCommand::Shell(_) => Ok(false),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::Editor;
  use crate::editor::command_registry::{RegisteredCommand, classify_command};
  use std::io;

  #[test]
  fn ocr_command_requires_exact_token_and_on_off_argument() {
    assert_eq!(classify_command("ocr on"), RegisteredCommand::Ocr(true));
    assert_eq!(classify_command("ocr off"), RegisteredCommand::Ocr(false));
    assert_eq!(classify_command("ocron"), RegisteredCommand::Unknown);
    assert_eq!(classify_command("ocrx"), RegisteredCommand::Unknown);
    assert_eq!(classify_command("ocr on now"), RegisteredCommand::Unknown);
  }

  #[test]
  fn ocr_prefixes_do_not_dispatch_as_ocr_commands() {
    let mut stdout = io::stdout();
    for command in ["ocron", "ocrx"] {
      let mut editor = Editor::new(vec!["line".to_string()], 80);
      editor.buffers[0].command_buffer = command.to_string();

      editor
        .execute_command(&mut stdout)
        .expect("command dispatch should not fail");

      assert!(!editor.ocr_enabled);
      assert_eq!(editor.buffers.len(), 1);
    }
  }
}

// Handle Vim-style commands
pub fn handle_command(command: &str, show_highlighter: &mut bool) -> bool {
  match command.trim() {
    "q" => true,
    "z" => {
      *show_highlighter = !*show_highlighter;
      false
    }
    "p" | "help" | "tutorial" => false,
    _ => false,
  }
}
