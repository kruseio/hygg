use crossterm::event::{self, Event as CEvent, KeyCode, KeyModifiers};
use std::io;

use super::command_registry::complete_command;
use super::core::{Editor, EditorMode};

impl Editor {
  fn clear_command_completion(&mut self) {
    self.editor_state.command_completion = None;
  }

  fn set_command_completion(&mut self, suggestions: Vec<&'static str>) {
    self.editor_state.command_completion =
      (!suggestions.is_empty()).then(|| suggestions.join(" "));
  }

  fn set_active_command_text_and_cursor(
    &mut self,
    command: String,
    cursor_pos: usize,
  ) {
    let cursor_pos = char_boundary_at_or_before(&command, cursor_pos);
    self.editor_state.command_buffer = command.clone();
    self.editor_state.command_cursor_pos = cursor_pos;
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.command_buffer = command;
      buffer.command_cursor_pos = cursor_pos;
    }
  }

  fn clear_active_command_text(&mut self) {
    self.clear_command_completion();
    self.set_active_command_text_and_cursor(String::new(), 0);
  }

  fn clear_all_command_text(&mut self, mode: EditorMode) {
    self.clear_command_completion();
    self.editor_state.command_buffer.clear();
    self.editor_state.command_cursor_pos = 0;
    self.editor_state.mode = mode.clone();
    for buffer in &mut self.buffers {
      buffer.command_buffer.clear();
      buffer.command_cursor_pos = 0;
      buffer.mode = mode.clone();
    }
  }

  fn set_active_command_cursor(&mut self, cursor_pos: usize) {
    let cursor_pos =
      char_boundary_at_or_before(&self.editor_state.command_buffer, cursor_pos);
    self.editor_state.command_cursor_pos = cursor_pos;
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.command_cursor_pos = cursor_pos;
    }
  }

  fn previous_active_command_cursor(&self) -> usize {
    let command = &self.editor_state.command_buffer;
    let pos =
      char_boundary_at_or_before(command, self.editor_state.command_cursor_pos);
    if pos == 0 {
      return 0;
    }
    command[..pos].char_indices().last().map(|(idx, _)| idx).unwrap_or(0)
  }

  fn next_active_command_cursor(&self) -> usize {
    let command = &self.editor_state.command_buffer;
    let pos =
      char_boundary_at_or_before(command, self.editor_state.command_cursor_pos);
    if pos >= command.len() {
      return command.len();
    }
    command[pos..]
      .chars()
      .next()
      .map(|c| pos + c.len_utf8())
      .unwrap_or(command.len())
  }

  fn insert_active_command_text(&mut self, text: &str) {
    self.clear_command_completion();
    let pos = char_boundary_at_or_before(
      &self.editor_state.command_buffer,
      self.editor_state.command_cursor_pos,
    );
    let mut command = self.editor_state.command_buffer.clone();
    command.insert_str(pos, text);
    self.set_active_command_text_and_cursor(command, pos + text.len());
  }

  fn insert_active_command_char(&mut self, c: char) {
    self.clear_command_completion();
    let pos = char_boundary_at_or_before(
      &self.editor_state.command_buffer,
      self.editor_state.command_cursor_pos,
    );
    let mut command = self.editor_state.command_buffer.clone();
    command.insert(pos, c);
    self.set_active_command_text_and_cursor(command, pos + c.len_utf8());
  }

  fn remove_active_command_char(&mut self, pos: usize) {
    self.clear_command_completion();
    let mut command = self.editor_state.command_buffer.clone();
    let start = char_boundary_at_or_before(&command, pos);
    if start >= command.len() {
      return;
    }
    let end = command[start..]
      .chars()
      .next()
      .map(|c| start + c.len_utf8())
      .unwrap_or(command.len());
    command.replace_range(start..end, "");
    let cursor_pos = char_boundary_at_or_before(
      &command,
      self.editor_state.command_cursor_pos.min(command.len()),
    );
    self.set_active_command_text_and_cursor(command, cursor_pos);
  }

  fn handle_command_completion(&mut self) {
    if self.get_active_mode() == EditorMode::CommandExecution {
      self.clear_command_completion();
      self.mark_dirty();
      return;
    }

    let command = self.get_active_command_buffer().to_string();
    let cursor_pos = self.get_active_command_cursor_pos();
    if cursor_pos != command.len() {
      self.clear_command_completion();
      self.mark_dirty();
      return;
    }

    let completion = complete_command(&command);
    if let Some(replacement) = completion.replacement {
      let cursor_pos = replacement.len();
      self.clear_command_completion();
      self.set_active_command_text_and_cursor(replacement, cursor_pos);
    } else {
      self.set_command_completion(completion.suggestions);
    }
    self.mark_dirty();
  }

  // Handle key events in command mode
  pub fn handle_command_mode_event(
    &mut self,
    key_event: event::KeyEvent,
    stdout: &mut io::Stdout,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    self.debug_log(&format!("handle_command_mode_event: key={key_event:?}"));
    self.debug_log(&format!(
      "  Command buffer: '{}', cursor_pos: {}",
      self.editor_state.command_buffer, self.editor_state.command_cursor_pos
    ));

    // Handle Ctrl+C to exit to normal mode
    if key_event.code == KeyCode::Char('c')
      && key_event.modifiers.contains(KeyModifiers::CONTROL)
    {
      self.debug_log("  Ctrl+C pressed, exiting to Normal mode");
      self.set_active_mode(EditorMode::Normal);
      self.clear_active_command_text();
      self.editor_state.visual_selection_active = false;
      self.mark_dirty();
      return Ok(false);
    }

    match key_event.code {
      KeyCode::Esc => {
        self.set_active_mode(EditorMode::Normal);
        self.editor_state.visual_selection_active = false;
        self.editor_state.previous_visual_mode = None;
        self.clear_all_command_text(EditorMode::Normal);
        self.mark_dirty(); // Force redraw to clear command line
      }
      KeyCode::Enter => {
        self.debug_log("  Enter pressed, executing command");
        self.clear_command_completion();
        let should_exit = self.execute_command(stdout)?;
        self.debug_log(&format!("  execute_command returned: {should_exit}"));
        if should_exit {
          return Ok(true);
        }
        // Ensure we're not leaving any command state behind
        self.debug_log("  Setting mode to Normal after command execution");
        self.set_active_mode(EditorMode::Normal);
        self.clear_all_command_text(EditorMode::Normal);
        self.mark_dirty(); // Force redraw to clear command line
      }
      KeyCode::Backspace => {
        if self.editor_state.command_cursor_pos > 0 {
          let pos = self.previous_active_command_cursor();
          self.set_active_command_cursor(pos);
          self.remove_active_command_char(pos);
        } else if self.editor_state.command_buffer.is_empty() {
          // If we're at position 0 and buffer is empty, we're trying to delete
          // the ':' Return to normal mode
          self.set_active_mode(EditorMode::Normal);
          self.clear_active_command_text();
          self.editor_state.visual_selection_active = false;
          self.mark_dirty();
        }
      }
      KeyCode::Delete => {
        if self.editor_state.command_cursor_pos
          < self.editor_state.command_buffer.len()
        {
          let pos = self.editor_state.command_cursor_pos;
          self.remove_active_command_char(pos);
        }
      }
      KeyCode::Left => {
        if self.editor_state.command_cursor_pos > 0 {
          self.set_active_command_cursor(self.previous_active_command_cursor());
        }
      }
      KeyCode::Right => {
        if self.editor_state.command_cursor_pos
          < self.editor_state.command_buffer.len()
        {
          self.set_active_command_cursor(self.next_active_command_cursor());
        }
      }
      KeyCode::Home => {
        self.set_active_command_cursor(0);
      }
      KeyCode::End => {
        self.set_active_command_cursor(self.editor_state.command_buffer.len());
      }
      KeyCode::Tab => {
        self.handle_command_completion();
      }
      KeyCode::Char('r')
        if key_event.modifiers.contains(KeyModifiers::CONTROL) =>
      {
        // Ctrl+R in command mode - paste from register
        if let CEvent::Key(register_key) = event::read()?
          && let KeyCode::Char('0') = register_key.code
        {
          // Paste from yank buffer (register 0) at cursor position
          let pos = self.editor_state.command_cursor_pos;
          let yank_text = self.editor_state.yank_buffer.clone();
          self.debug_log_event(
            "command_mode",
            "paste_register_0",
            &format!("yank_buffer='{yank_text}', pos={pos}"),
          );

          // Remove newlines from yanked text to prevent command execution
          let clean_text = yank_text.replace('\n', " ").replace('\r', "");
          self.debug_log_state("command_mode", "clean_text", &clean_text);

          self.insert_active_command_text(&clean_text);

          // Track paste for tutorial
          if self.tutorial_active {
            self.tutorial_paste_performed = true;
            self.debug_log("Tutorial: paste performed via Ctrl+R 0");
          }

          self.debug_log_state(
            "command_mode",
            "new_command_buffer",
            &self.editor_state.command_buffer,
          );
        }
        // Other registers not implemented yet
      }
      KeyCode::Char('v')
        if key_event.modifiers.contains(KeyModifiers::CONTROL) =>
      {
        // Ctrl+V in command mode - paste from system clipboard
        if let Some(clipboard) = &mut self.clipboard
          && let Ok(clipboard_text) = clipboard.get_text()
        {
          // Remove newlines from clipboard text to prevent command execution
          let clean_text = clipboard_text.replace('\n', " ").replace('\r', "");
          self.insert_active_command_text(&clean_text);

          // Track paste for tutorial
          if self.tutorial_active {
            self.tutorial_paste_performed = true;
            self.debug_log("Tutorial: paste performed via Ctrl+V");
          }

          self.debug_log(&format!("Pasted from clipboard: '{clipboard_text}'"));
        }
      }
      KeyCode::Char(c) => {
        // Check for '!' at start of command to enter CommandExecution mode
        if c == '!'
          && self.editor_state.command_buffer.is_empty()
          && self.get_active_mode() == EditorMode::Command
        {
          self.set_active_mode(EditorMode::CommandExecution);
          self.debug_log("Entering CommandExecution mode");
        }
        let pos = self.editor_state.command_cursor_pos;
        self.insert_active_command_char(c);

        self.debug_log(&format!(
          "  Added '{}' at position {}, buffer='{}'",
          c, pos, self.editor_state.command_buffer
        ));
      }
      _ => {
        self.debug_log(&format!(
          "  Unhandled key in command mode: {:?}",
          key_event.code
        ));
      }
    }
    self.debug_log(&format!(
      "  Command mode event complete, mode={:?}",
      self.editor_state.mode
    ));
    Ok(false)
  }
}

fn char_boundary_at_or_before(value: &str, cursor_pos: usize) -> usize {
  let mut pos = cursor_pos.min(value.len());
  while pos > 0 && !value.is_char_boundary(pos) {
    pos -= 1;
  }
  pos
}

#[cfg(test)]
mod tests {
  use super::*;
  use crossterm::event::{KeyEvent, KeyEventKind, KeyEventState};

  fn key(code: KeyCode) -> event::KeyEvent {
    KeyEvent {
      code,
      modifiers: KeyModifiers::empty(),
      kind: KeyEventKind::Press,
      state: KeyEventState::empty(),
    }
  }

  fn ctrl(code: KeyCode) -> event::KeyEvent {
    KeyEvent {
      code,
      modifiers: KeyModifiers::CONTROL,
      kind: KeyEventKind::Press,
      state: KeyEventState::empty(),
    }
  }

  fn command_editor(command: &str) -> Editor {
    let mut editor = Editor::new(vec!["line".to_string()], 80);
    editor.set_active_mode(EditorMode::Command);
    editor
      .set_active_command_text_and_cursor(command.to_string(), command.len());
    editor
  }

  #[test]
  fn tab_on_empty_command_lists_top_level_commands() {
    let mut editor = command_editor("");
    let mut stdout = io::stdout();

    editor
      .handle_command_mode_event(key(KeyCode::Tab), &mut stdout)
      .expect("tab completion should succeed");

    let completion = editor
      .editor_state
      .command_completion
      .as_deref()
      .expect("completion should be visible");
    assert!(completion.contains("ocr"));
    assert!(completion.contains("tutorial"));
    assert_eq!(editor.get_active_command_buffer(), "");
  }

  #[test]
  fn repeated_empty_tab_keeps_completion_visible() {
    let mut editor = command_editor("");
    let mut stdout = io::stdout();

    editor
      .handle_command_mode_event(key(KeyCode::Tab), &mut stdout)
      .expect("first tab should succeed");
    let first = editor.editor_state.command_completion.clone();
    editor
      .handle_command_mode_event(key(KeyCode::Tab), &mut stdout)
      .expect("second tab should succeed");

    assert_eq!(editor.editor_state.command_completion, first);
  }

  #[test]
  fn unique_prefix_completes_command_buffer_and_active_buffer() {
    let mut editor = command_editor("abo");
    let mut stdout = io::stdout();

    editor
      .handle_command_mode_event(key(KeyCode::Tab), &mut stdout)
      .expect("tab completion should succeed");

    assert_eq!(editor.editor_state.command_buffer, "about");
    assert_eq!(editor.editor_state.command_cursor_pos, 5);
    assert_eq!(editor.buffers[0].command_buffer, "about");
    assert_eq!(editor.buffers[0].command_cursor_pos, 5);
    assert!(editor.editor_state.command_completion.is_none());
  }

  #[test]
  fn ocr_tab_lists_arguments_without_editing_command() {
    let mut editor = command_editor("ocr ");
    let mut stdout = io::stdout();

    editor
      .handle_command_mode_event(key(KeyCode::Tab), &mut stdout)
      .expect("tab completion should succeed");

    assert_eq!(editor.get_active_command_buffer(), "ocr ");
    assert_eq!(
      editor.editor_state.command_completion.as_deref(),
      Some("on off")
    );
  }

  #[test]
  fn delete_updates_editor_state_and_active_buffer() {
    let mut editor = command_editor("abcd");
    let mut stdout = io::stdout();
    editor.set_active_command_cursor(1);

    editor
      .handle_command_mode_event(key(KeyCode::Delete), &mut stdout)
      .expect("delete should succeed");

    assert_eq!(editor.editor_state.command_buffer, "acd");
    assert_eq!(editor.buffers[0].command_buffer, "acd");
    assert_eq!(editor.editor_state.command_cursor_pos, 1);
    assert_eq!(editor.buffers[0].command_cursor_pos, 1);
  }

  #[test]
  fn edit_clears_visible_completion() {
    let mut editor = command_editor("");
    let mut stdout = io::stdout();
    editor.editor_state.command_completion = Some("about author".to_string());

    editor
      .handle_command_mode_event(key(KeyCode::Char('a')), &mut stdout)
      .expect("edit should succeed");

    assert!(editor.editor_state.command_completion.is_none());
  }

  #[test]
  fn ctrl_c_clears_visible_completion() {
    let mut editor = command_editor("");
    let mut stdout = io::stdout();
    editor.editor_state.command_completion = Some("about author".to_string());

    editor
      .handle_command_mode_event(ctrl(KeyCode::Char('c')), &mut stdout)
      .expect("ctrl-c should succeed");

    assert!(editor.editor_state.command_completion.is_none());
    assert_eq!(editor.get_active_mode(), EditorMode::Normal);
  }

  #[test]
  fn command_execution_tab_does_not_complete_shell_command() {
    let mut editor = command_editor("!ec");
    editor.set_active_mode(EditorMode::CommandExecution);
    let mut stdout = io::stdout();

    editor
      .handle_command_mode_event(key(KeyCode::Tab), &mut stdout)
      .expect("tab should be ignored");

    assert_eq!(editor.get_active_command_buffer(), "!ec");
    assert!(editor.editor_state.command_completion.is_none());
  }

  #[test]
  fn unicode_command_editing_uses_char_boundaries() {
    let mut editor = command_editor("éx");
    let mut stdout = io::stdout();

    editor.set_active_command_cursor("éx".len());
    editor
      .handle_command_mode_event(key(KeyCode::Left), &mut stdout)
      .expect("left should succeed");
    editor
      .handle_command_mode_event(key(KeyCode::Backspace), &mut stdout)
      .expect("backspace should succeed");

    assert_eq!(editor.get_active_command_buffer(), "x");
    assert_eq!(editor.get_active_command_cursor_pos(), 0);
    assert_eq!(editor.buffers[0].command_buffer, "x");
    assert_eq!(editor.buffers[0].command_cursor_pos, 0);
  }
}
