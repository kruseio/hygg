use super::super::command_registry::complete_command;
use super::super::core::{Editor, EditorMode};

impl Editor {
  pub(crate) fn clear_command_completion(&mut self) {
    self.editor_state.command_completion = None;
  }

  pub(crate) fn set_command_completion(
    &mut self,
    suggestions: Vec<&'static str>,
  ) {
    self.editor_state.command_completion =
      (!suggestions.is_empty()).then(|| suggestions.join(" "));
  }

  pub(crate) fn set_active_command_text_and_cursor(
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

  pub(crate) fn clear_active_command_text(&mut self) {
    self.clear_command_completion();
    self.set_active_command_text_and_cursor(String::new(), 0);
  }

  pub(crate) fn clear_all_command_text(&mut self, mode: EditorMode) {
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

  pub(crate) fn set_active_command_cursor(&mut self, cursor_pos: usize) {
    let cursor_pos =
      char_boundary_at_or_before(&self.editor_state.command_buffer, cursor_pos);
    self.editor_state.command_cursor_pos = cursor_pos;
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.command_cursor_pos = cursor_pos;
    }
  }

  pub(crate) fn previous_active_command_cursor(&self) -> usize {
    let command = &self.editor_state.command_buffer;
    let pos =
      char_boundary_at_or_before(command, self.editor_state.command_cursor_pos);
    if pos == 0 {
      return 0;
    }
    command[..pos].char_indices().last().map(|(idx, _)| idx).unwrap_or(0)
  }

  pub(crate) fn next_active_command_cursor(&self) -> usize {
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

  pub(crate) fn insert_active_command_text(&mut self, text: &str) {
    self.clear_command_completion();
    let pos = char_boundary_at_or_before(
      &self.editor_state.command_buffer,
      self.editor_state.command_cursor_pos,
    );
    let mut command = self.editor_state.command_buffer.clone();
    command.insert_str(pos, text);
    self.set_active_command_text_and_cursor(command, pos + text.len());
  }

  pub(crate) fn insert_active_command_char(&mut self, c: char) {
    self.clear_command_completion();
    let pos = char_boundary_at_or_before(
      &self.editor_state.command_buffer,
      self.editor_state.command_cursor_pos,
    );
    let mut command = self.editor_state.command_buffer.clone();
    command.insert(pos, c);
    self.set_active_command_text_and_cursor(command, pos + c.len_utf8());
  }

  pub(crate) fn remove_active_command_char(&mut self, pos: usize) {
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

  pub(crate) fn handle_command_completion(&mut self) {
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
}

pub(crate) fn char_boundary_at_or_before(
  value: &str,
  cursor_pos: usize,
) -> usize {
  let mut pos = cursor_pos.min(value.len());
  while pos > 0 && !value.is_char_boundary(pos) {
    pos -= 1;
  }
  pos
}
