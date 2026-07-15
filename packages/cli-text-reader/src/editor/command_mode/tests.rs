use std::io;

use crossterm::event::{
  self, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers,
};

use super::super::core::{Editor, EditorMode};

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
  editor.set_active_command_text_and_cursor(command.to_string(), command.len());
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
  assert_eq!(editor.editor_state.command_completion.as_deref(), Some("on off"));
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
