use super::char_boundary::{CursorCol, DisplayCol};
use super::core::Editor;
use crossterm::event::{KeyCode, KeyModifiers};

impl Editor {
  // Handle movement keys in visual mode with selection updates
  pub fn handle_visual_movement_keys(
    &mut self,
    key_code: KeyCode,
    modifiers: KeyModifiers,
  ) -> Result<Option<bool>, Box<dyn std::error::Error>> {
    match key_code {
      KeyCode::Char('j') | KeyCode::Down => {
        self.move_cursor_down();
        self.update_selection();
        Ok(Some(false))
      }
      KeyCode::Char('k') | KeyCode::Up => {
        self.move_cursor_up();
        self.update_selection();
        Ok(Some(false))
      }
      KeyCode::Char('h') | KeyCode::Left => {
        self.move_cursor_left();
        self.update_selection();
        Ok(Some(false))
      }
      KeyCode::Char('l') | KeyCode::Right => {
        self.move_cursor_right();
        self.update_selection();
        Ok(Some(false))
      }
      KeyCode::Char('w') => {
        // Word forward motion
        self.move_to_word_boundary(true, false);
        self.update_selection();
        Ok(Some(false))
      }
      KeyCode::Char('W') => {
        // Big word forward motion
        self.move_to_word_boundary(true, true);
        self.update_selection();
        Ok(Some(false))
      }
      KeyCode::Char('b') => {
        // Word backward motion
        self.move_to_word_boundary(false, false);
        self.update_selection();
        Ok(Some(false))
      }
      KeyCode::Char('B') => {
        // Big word backward motion
        self.move_to_word_boundary(false, true);
        self.update_selection();
        Ok(Some(false))
      }
      KeyCode::Char('e') => {
        // End of word motion in visual mode
        let target = self.find_word_end(false);
        self.move_to_position(target);
        self.update_selection();
        Ok(Some(false))
      }
      KeyCode::Char('E') => {
        // End of big word motion in visual mode
        let target = self.find_word_end(true);
        self.move_to_position(target);
        self.update_selection();
        Ok(Some(false))
      }
      KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
        // Ctrl+U - half page up in visual mode
        self.half_page_up();
        self.update_selection();
        Ok(Some(false))
      }
      KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => {
        // Ctrl+D - half page down in visual mode
        self.half_page_down();
        self.update_selection();
        Ok(Some(false))
      }
      KeyCode::Char('0') => {
        // '0' - jump to start of line in visual mode
        self.cursor_x = CursorCol::ZERO;
        self.update_selection();
        Ok(Some(false))
      }
      KeyCode::Char('$') => {
        // '$' - jump to end of line in visual mode
        let current_line_idx = self.offset + self.cursor_y;
        if current_line_idx < self.lines.len() {
          // Start of the last visible character, not `len() - 1`, which
          // lands inside a trailing multi-byte character (issue #99).
          self.cursor_x = CursorCol::last(&self.lines[current_line_idx]);
          // Like vim, `$` makes j/k stay at the end of every line.
          self.remember_display_column(DisplayCol::END);
        }
        self.update_selection();
        Ok(Some(false))
      }
      KeyCode::Char('^') => {
        // '^' - jump to first non-whitespace character in visual mode
        let current_line_idx = self.offset + self.cursor_y;
        if current_line_idx < self.lines.len() {
          // The start if the line is all whitespace
          self.cursor_x =
            CursorCol::first_non_blank(&self.lines[current_line_idx]);
          self.update_selection();
        }
        Ok(Some(false))
      }
      KeyCode::Char('%') => {
        // Jump to matching bracket/parenthesis in visual mode
        if let Some(target) = self.find_matching_bracket() {
          self.move_to_position(target);
          self.update_selection();
        }
        Ok(Some(false))
      }
      _ => Ok(None),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::editor::char_boundary::Pos;
  use crossterm::event::KeyEvent;

  const LINE: &str = "La canción está aquí";

  fn key(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
  }

  fn visual_editor(lines: &[&str]) -> Editor {
    let mut editor =
      Editor::new(lines.iter().map(|l| l.to_string()).collect(), 80);
    editor.height = 10;
    editor.offset = 0;
    editor.cursor_y = 0;
    editor.handle_normal_mode_event(key('v')).unwrap();
    editor
  }

  fn assert_selection_on_boundaries(editor: &Editor) {
    for (row, col) in [
      editor.editor_state.selection_start.map(Pos::to_raw),
      editor.editor_state.selection_end.map(Pos::to_raw),
    ]
    .into_iter()
    .flatten()
    {
      assert!(editor.lines[row].is_char_boundary(col), "({row}, {col})");
    }
  }

  #[test]
  fn visual_l_and_h_step_whole_characters() {
    let mut editor = visual_editor(&[LINE]);
    let starts: Vec<usize> = LINE.char_indices().map(|(i, _)| i).collect();

    let mut seen = vec![editor.cursor_x.raw()];
    for _ in 0..starts.len() + 2 {
      editor.handle_visual_mode_event(key('l')).unwrap();
      assert_selection_on_boundaries(&editor);
      if seen.last() != Some(&editor.cursor_x.raw()) {
        seen.push(editor.cursor_x.raw());
      }
    }
    assert_eq!(seen, starts);

    // From 'í' (21) back over 'u' (20), 'q' (19), then the space at 17
    // and onto 'á' (15).
    for expected in [20, 19, 18, 17, 15] {
      editor.handle_visual_mode_event(key('h')).unwrap();
      assert_eq!(editor.cursor_x.raw(), expected);
    }
    assert_selection_on_boundaries(&editor);
  }

  #[test]
  fn visual_dollar_and_zero_on_multibyte_lines() {
    let mut editor = visual_editor(&[LINE, "€€"]);
    editor.handle_visual_mode_event(key('$')).unwrap();
    assert_eq!(editor.cursor_x.raw(), 21);
    assert_selection_on_boundaries(&editor);
    editor.handle_visual_mode_event(key('j')).unwrap();
    assert_eq!(editor.cursor_x.raw(), 3);
    editor.handle_visual_mode_event(key('0')).unwrap();
    assert_eq!(editor.cursor_x.raw(), 0);
    editor.handle_visual_mode_event(key('$')).unwrap();
    assert_eq!(editor.cursor_x.raw(), 3);
    assert_selection_on_boundaries(&editor);
  }

  #[test]
  fn visual_word_motions_on_accented_text() {
    let mut editor = visual_editor(&[LINE]);
    editor.handle_visual_mode_event(key('e')).unwrap();
    assert_eq!(editor.cursor_x.raw(), 1);
    editor.handle_visual_mode_event(key('w')).unwrap();
    assert_eq!(editor.cursor_x.raw(), 3);
    editor.handle_visual_mode_event(key('e')).unwrap();
    assert_eq!(editor.cursor_x.raw(), 10);
    editor.handle_visual_mode_event(key('w')).unwrap();
    assert_eq!(editor.cursor_x.raw(), 12);
    editor.handle_visual_mode_event(key('E')).unwrap();
    assert_eq!(editor.cursor_x.raw(), 15);
    editor.handle_visual_mode_event(key('b')).unwrap();
    assert_eq!(editor.cursor_x.raw(), 12);
    assert_selection_on_boundaries(&editor);
  }

  #[test]
  fn visual_l_and_dollar_step_whole_graphemes() {
    let line = "e\u{301}ที่👩\u{200d}💻🇪🇸中";
    let mut editor = visual_editor(&[line, "Ａb"]);
    let mut seen = vec![editor.cursor_x.raw()];
    for _ in 0..8 {
      editor.handle_visual_mode_event(key('l')).unwrap();
      if seen.last() != Some(&editor.cursor_x.raw()) {
        seen.push(editor.cursor_x.raw());
      }
      assert_selection_on_boundaries(&editor);
    }
    assert_eq!(seen, vec![0, 3, 12, 23, 31]);
    editor.handle_visual_mode_event(key('0')).unwrap();
    editor.handle_visual_mode_event(key('$')).unwrap();
    assert_eq!(editor.cursor_x.raw(), 31);
    // `$` sticks to the end through j.
    editor.handle_visual_mode_event(key('j')).unwrap();
    assert_eq!(editor.cursor_x.raw(), 3);
    // h then l sets the column to 'b' (display column 2, after the
    // fullwidth 'Ａ'). Up, column 2 is the ZWJ emoji: e+accent and ที่ take
    // one cell each.
    editor.handle_visual_mode_event(key('h')).unwrap();
    editor.handle_visual_mode_event(key('l')).unwrap();
    editor.handle_visual_mode_event(key('k')).unwrap();
    assert_eq!(editor.cursor_x.raw(), 12);
    assert_selection_on_boundaries(&editor);
  }
}
