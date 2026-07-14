//! Custom text-field state + editing for the Settings credential inputs. The
//! app owns each field's value, caret and selection anchor (unlike iced's
//! `text_input`), so the menu can copy the selection and paste at the caret.
//! Keystrokes arrive as [`Message::KeyPressed`] and dispatch here while
//! focused.

use iced::Task;
use iced::keyboard::key::Named;
use iced::keyboard::{Key, Modifiers};

use super::{HyggGui, Message};

/// Which credential field. Values live in `Settings`/`Account`; editors here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldId {
  ServerUrl,
  User,
  Token,
}

/// A field's caret + selection, in character indices (`anchor..cursor`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Editor {
  pub cursor: usize,
  pub anchor: usize,
}

impl Editor {
  /// The ordered selection range `(start, end)`.
  pub fn range(&self) -> (usize, usize) {
    (self.cursor.min(self.anchor), self.cursor.max(self.anchor))
  }
}

/// A field's undo/redo stacks of `(value, editor)` snapshots.
#[derive(Default)]
pub struct History {
  pub undo: Vec<(String, Editor)>,
  pub redo: Vec<(String, Editor)>,
}

/// Per-field editors, focus, mouse-selection + click state, and undo history.
#[derive(Default)]
pub struct Fields {
  pub server: Editor,
  pub user: Editor,
  pub token: Editor,
  pub focused: Option<FieldId>,
  selecting: bool,
  hover_x: f32,
  /// Consecutive-click accounting for double-click word / triple-click line.
  last_click_ms: f64,
  last_click: Option<(FieldId, usize)>,
  click_count: u8,
  server_h: History,
  user_h: History,
  token_h: History,
}

/// Field interaction messages (mouse + resolved paste).
#[derive(Debug, Clone)]
pub enum FieldMsg {
  /// Mouse pressed over a field — focus it and drop the caret at the pointer.
  Press(FieldId),
  /// Pointer moved over a field (x from its text start); extends a drag select.
  Move(FieldId, f32),
  /// Mouse released — ends a selection drag.
  Release,
  /// Clipboard text resolved for a paste — insert it at the caret.
  Pasted(FieldId, String),
}

impl HyggGui {
  /// The current text of a field (borrowed from `Settings`/`Account`).
  pub fn field_value(&self, fid: FieldId) -> &str {
    match fid {
      FieldId::ServerUrl => &self.settings.server_url,
      FieldId::User => &self.account.user,
      FieldId::Token => &self.account.token,
    }
  }

  /// The field's caret/selection editor.
  pub fn editor(&self, fid: FieldId) -> Editor {
    match fid {
      FieldId::ServerUrl => self.fields.server,
      FieldId::User => self.fields.user,
      FieldId::Token => self.fields.token,
    }
  }

  /// Whether `fid` currently holds keyboard focus.
  pub fn field_focused(&self, fid: FieldId) -> bool {
    self.fields.focused == Some(fid)
  }

  /// Drop text-field focus (leaving Settings) so keystrokes go to the reader.
  pub(super) fn blur_field(&mut self) {
    self.fields.focused = None;
    self.fields.selecting = false;
  }

  pub(super) fn editor_mut(&mut self, fid: FieldId) -> &mut Editor {
    match fid {
      FieldId::ServerUrl => &mut self.fields.server,
      FieldId::User => &mut self.fields.user,
      FieldId::Token => &mut self.fields.token,
    }
  }

  pub(super) fn history_mut(&mut self, fid: FieldId) -> &mut History {
    match fid {
      FieldId::ServerUrl => &mut self.fields.server_h,
      FieldId::User => &mut self.fields.user_h,
      FieldId::Token => &mut self.fields.token_h,
    }
  }

  pub(super) fn set_field_value(&mut self, fid: FieldId, v: String) {
    match fid {
      FieldId::ServerUrl => {
        self.settings.server_url = v;
        self.settings.save();
      }
      FieldId::User => self.account.user = v,
      FieldId::Token => self.account.token = v,
    }
  }

  /// Handle a field mouse / paste message.
  pub fn field_update(&mut self, m: FieldMsg) -> Task<Message> {
    match m {
      FieldMsg::Press(fid) => {
        let len = self.field_value(fid).chars().count();
        let idx = crate::screens::field::char_index(self.fields.hover_x, len);
        // Consecutive clicks: 2 = word, 3 = whole field (line). Like the
        // reader.
        let now = crate::util::now_ms();
        let repeat = now - self.fields.last_click_ms < 450.0
          && self.fields.last_click.is_some_and(|(f, i)| {
            f == fid && (i as i64 - idx as i64).abs() <= 1
          });
        let count =
          if repeat { (self.fields.click_count + 1).min(3) } else { 1 };
        self.fields.click_count = count;
        self.fields.last_click_ms = now;
        self.fields.last_click = Some((fid, idx));
        self.fields.focused = Some(fid);
        let word = crate::select::word_bounds(self.field_value(fid), idx);
        let (anchor, cursor, selecting) = match count {
          2 => (word.0, word.1, false),
          3 => (0, len, false),
          _ => (idx, idx, true),
        };
        self.fields.selecting = selecting;
        let ed = self.editor_mut(fid);
        ed.anchor = anchor;
        ed.cursor = cursor;
      }
      FieldMsg::Move(fid, x) => {
        self.fields.hover_x = x;
        if self.fields.selecting && self.fields.focused == Some(fid) {
          let len = self.field_value(fid).chars().count();
          self.editor_mut(fid).cursor =
            crate::screens::field::char_index(x, len);
        }
      }
      FieldMsg::Release => self.fields.selecting = false,
      FieldMsg::Pasted(fid, content) => self.field_insert(fid, &content),
    }
    Task::none()
  }

  /// Dispatch a keystroke to the focused field.
  pub fn field_key_input(
    &mut self,
    fid: FieldId,
    key: Key,
    text: Option<String>,
    mods: Modifiers,
  ) -> Task<Message> {
    // Clipboard / undo — the platform modifier (`command()` = Cmd or Ctrl) plus
    // a letter. Backspace / arrows also take the command modifier (line-wise),
    // so those fall through to the editing match below.
    if mods.command() && matches!(key.as_ref(), Key::Character(_)) {
      return match key.as_ref() {
        Key::Character("c") => self.field_copy(fid),
        Key::Character("x") => self.field_cut(fid),
        Key::Character("v") => self.field_paste(fid),
        Key::Character("a") => {
          self.field_select_all(fid);
          Task::none()
        }
        Key::Character("z" | "Z") if mods.shift() => {
          self.field_redo(fid);
          Task::none()
        }
        Key::Character("z" | "Z") => {
          self.field_undo(fid);
          Task::none()
        }
        _ => Task::none(),
      };
    }
    // `word` (Option / Ctrl) and `line` (Cmd) granularity for Backspace /
    // arrows.
    let (word, line) = edit_granularity(mods);
    match key {
      Key::Named(Named::Backspace) => self.field_erase(fid, false, word, line),
      Key::Named(Named::Delete) => self.field_erase(fid, true, word, line),
      Key::Named(Named::ArrowLeft) => {
        self.field_move(fid, -1, mods.shift(), word, line)
      }
      Key::Named(Named::ArrowRight) => {
        self.field_move(fid, 1, mods.shift(), word, line)
      }
      Key::Named(Named::Home) => self.field_edge(fid, false, mods.shift()),
      Key::Named(Named::End) => self.field_edge(fid, true, mods.shift()),
      Key::Named(Named::Enter) => {
        self.fields.focused = None;
      }
      _ => {
        if !mods.command()
          && let Some(t) = text
          && !t.is_empty()
          && !t.chars().any(|c| c.is_control())
        {
          self.field_insert(fid, &t);
        }
      }
    }
    Task::none()
  }

  /// Apply an edit to a field's characters + editor, snapshotting the pre-edit
  /// state for undo, then persist the new value.
  pub(super) fn field_edit(
    &mut self,
    fid: FieldId,
    f: impl FnOnce(&mut Vec<char>, &mut Editor),
  ) {
    self.push_undo(fid);
    let mut chars: Vec<char> = self.field_value(fid).chars().collect();
    let mut ed = self.editor(fid);
    f(&mut chars, &mut ed);
    self.set_field_value(fid, chars.into_iter().collect());
    *self.editor_mut(fid) = ed;
  }
}

/// The `(word, line)` editing granularity for a modifier combo, matching the
/// platform's browser conventions: on macOS, Option = word and Cmd = line; on
/// Windows / Linux, Ctrl = word (line is via the Home/End keys).
fn edit_granularity(mods: Modifiers) -> (bool, bool) {
  if cfg!(target_os = "macos") {
    (mods.alt(), mods.command())
  } else {
    (mods.control(), false)
  }
}
