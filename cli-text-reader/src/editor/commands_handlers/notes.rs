use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::super::core::{Editor, EditorMode};
use crate::notes::{Note, save_notes};

impl Editor {
  /// `:note` — open the notes overlay for the current document. Existing notes
  /// are listed; typing appends to a new note, Enter saves it, Esc closes.
  pub fn handle_note_command(
    &mut self,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    // Anchor new notes to the document line the cursor was on when opened.
    self.notes_anchor = Some(self.offset + self.cursor_y);
    self.notes_input.clear();
    self.notes_active = true;

    let lines = self.render_notes_overlay_lines();
    self.create_overlay("note", lines);

    // If the overlay could not be created, do not trap keystrokes.
    if self.buffers.len() <= 1 {
      self.notes_active = false;
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

  /// Keystroke handler used while the notes overlay is capturing input. Routed
  /// from `handle_normal_mode_event` before any normal-mode dispatch so typing
  /// never touches the document's vim state machine.
  pub fn handle_notes_input_key(
    &mut self,
    key_event: KeyEvent,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    let ctrl = key_event.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key_event.modifiers.contains(KeyModifiers::ALT);
    match key_event.code {
      KeyCode::Esc => self.close_notes_overlay(),
      KeyCode::Enter => self.commit_current_note(),
      KeyCode::Backspace => {
        self.notes_input.pop();
        self.refresh_notes_overlay();
      }
      KeyCode::Char('c') if ctrl => self.close_notes_overlay(),
      KeyCode::Char(c) if !ctrl && !alt => {
        self.notes_input.push(c);
        self.refresh_notes_overlay();
      }
      _ => {}
    }
    Ok(false)
  }

  /// Commit the in-progress note (if non-empty), persist all notes, and keep
  /// the overlay open for the next note.
  fn commit_current_note(&mut self) {
    let body = self.notes_input.trim().to_string();
    if body.is_empty() {
      return;
    }
    let note = Note::new(body, self.notes_anchor);
    self.enqueue_note_sync(&note, false);
    self.notes.notes.push(note);
    if let Err(e) = save_notes(self.document_hash, &self.notes) {
      self.debug_log_error(&format!("Failed to save notes: {e}"));
    }
    self.notes_input.clear();
    self.refresh_notes_overlay();
  }

  fn close_notes_overlay(&mut self) {
    self.notes_active = false;
    self.notes_input.clear();
    self.notes_anchor = None;
    self.close_overlay();
    self.set_active_mode(EditorMode::Normal);
    self.mark_dirty();
  }

  fn refresh_notes_overlay(&mut self) {
    let lines = self.render_notes_overlay_lines();
    self.create_overlay("note", lines);
    self.set_active_mode(EditorMode::Normal);
    self.mark_dirty();
  }

  fn render_notes_overlay_lines(&self) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!("  Notes ({})", self.notes.notes.len()));
    lines.push("  ".to_string());
    if self.notes.notes.is_empty() {
      lines.push("  (no notes yet)".to_string());
    } else {
      for (idx, note) in self.notes.notes.iter().enumerate() {
        lines.push(format!("  {}. {}", idx + 1, note.body));
      }
    }
    lines.push("  ".to_string());
    lines.push("  Type a note · Enter saves · Esc closes".to_string());
    lines.push(format!("  > {}", self.notes_input));
    lines
  }
}
