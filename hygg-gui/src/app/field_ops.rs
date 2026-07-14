//! Field editing operations — insert / erase / caret movement / select-all,
//! clipboard (copy / cut / paste of the app-owned selection), and undo/redo.
//! Split out of `fields.rs` for the source LOC budget; more `impl HyggGui`
//! methods, reached from `field_key_input` and the context menu.

use iced::Task;

use super::{FieldId, FieldMsg, HyggGui, Message};

impl HyggGui {
  /// The selected substring of a field, or `None` when nothing is selected.
  pub fn field_selection_text(&self, fid: FieldId) -> Option<String> {
    let (a, b) = self.editor(fid).range();
    if a == b {
      return None;
    }
    Some(self.field_value(fid).chars().skip(a).take(b - a).collect())
  }

  /// Copy the field's selection to the clipboard.
  pub fn field_copy(&self, fid: FieldId) -> Task<Message> {
    match self.field_selection_text(fid) {
      Some(t) => iced::clipboard::write(t),
      None => Task::none(),
    }
  }

  /// Begin connecting this device to the sync server from the account form:
  /// validate the two fields, persist a stable machine id, then fire the `/me`
  /// check whose result [`Message::Connected`] adopts. Lives here
  /// (account/field flow) rather than bloating the reader-heavy update
  /// handler.
  pub(super) fn begin_connect(&mut self) -> Task<Message> {
    let username = self.account.user.trim().to_string();
    let token = self.account.token.trim().to_string();
    if username.is_empty() || token.is_empty() {
      self.account.status = "Enter your username and device token.".to_string();
      return Task::none();
    }
    // Persist a stable machine id before the request — the token binds to it.
    let machine_id = self.settings.ensure_machine_id();
    self.settings.save();
    let creds = crate::sync::Creds {
      server: self.settings.server_url.clone(),
      token: token.clone(),
      username: username.clone(),
      machine_id,
      device_id: String::new(),
    };
    self.account.busy = true;
    self.account.status = "Connecting…".to_string();
    Task::perform(
      async move {
        crate::sync::fetch_me(&creds).await.map(|me| (username, token, me))
      },
      Message::Connected,
    )
  }

  /// Copy the selection, then delete it.
  pub fn field_cut(&mut self, fid: FieldId) -> Task<Message> {
    let text = self.field_selection_text(fid);
    self.field_edit(fid, |chars, ed| {
      let (a, b) = ed.range();
      if a != b {
        chars.drain(a..b);
        ed.cursor = a;
        ed.anchor = a;
      }
    });
    match text {
      Some(t) => iced::clipboard::write(t),
      None => Task::none(),
    }
  }

  /// Read the clipboard and insert it at the field's caret.
  pub fn field_paste(&self, fid: FieldId) -> Task<Message> {
    iced::clipboard::read().map(move |c| {
      Message::Field(FieldMsg::Pasted(fid, c.unwrap_or_default()))
    })
  }

  /// Snapshot the field's current `(value, editor)` for undo and drop the redo
  /// stack (a fresh edit invalidates any redo). Called before each edit.
  pub(super) fn push_undo(&mut self, fid: FieldId) {
    let snap = (self.field_value(fid).to_string(), self.editor(fid));
    let h = self.history_mut(fid);
    h.undo.push(snap);
    h.redo.clear();
    if h.undo.len() > 200 {
      h.undo.remove(0);
    }
  }

  /// Undo the last edit, saving the current state to the redo stack.
  pub fn field_undo(&mut self, fid: FieldId) {
    let Some(prev) = self.history_mut(fid).undo.pop() else {
      return;
    };
    let cur = (self.field_value(fid).to_string(), self.editor(fid));
    self.history_mut(fid).redo.push(cur);
    self.set_field_value(fid, prev.0);
    *self.editor_mut(fid) = prev.1;
  }

  /// Redo a previously-undone edit.
  pub fn field_redo(&mut self, fid: FieldId) {
    let Some(next) = self.history_mut(fid).redo.pop() else {
      return;
    };
    let cur = (self.field_value(fid).to_string(), self.editor(fid));
    self.history_mut(fid).undo.push(cur);
    self.set_field_value(fid, next.0);
    *self.editor_mut(fid) = next.1;
  }

  /// Select the whole field.
  pub fn field_select_all(&mut self, fid: FieldId) {
    let len = self.field_value(fid).chars().count();
    let ed = self.editor_mut(fid);
    ed.anchor = 0;
    ed.cursor = len;
  }

  /// Replace the selection (or nothing) with `s`, leaving the caret after it.
  pub(super) fn field_insert(&mut self, fid: FieldId, s: &str) {
    let ins: Vec<char> = s.chars().collect();
    self.field_edit(fid, |chars, ed| {
      let (a, b) = ed.range();
      chars.splice(a..b, ins.iter().copied());
      ed.cursor = a + ins.len();
      ed.anchor = ed.cursor;
    });
  }

  /// Delete: the selection if any, else by `line` (to the field edge), `word`,
  /// or a single character, in the `forward` (Delete) or backward (Backspace)
  /// direction. Mirrors a browser's Cmd/Option/Ctrl + Backspace/Delete.
  pub(super) fn field_erase(
    &mut self,
    fid: FieldId,
    forward: bool,
    word: bool,
    line: bool,
  ) {
    self.field_edit(fid, |chars, ed| {
      let (a, b) = ed.range();
      if a != b {
        chars.drain(a..b);
        ed.cursor = a;
      } else {
        let to = edit_target(chars, ed.cursor, forward, word, line);
        let (lo, hi) = (ed.cursor.min(to), ed.cursor.max(to));
        chars.drain(lo..hi);
        ed.cursor = lo;
      }
      ed.anchor = ed.cursor;
    });
  }

  /// Move the caret by a character / `word` / `line` in direction `dir`
  /// (−1/＋1), extending the selection when `extend`.
  pub(super) fn field_move(
    &mut self,
    fid: FieldId,
    dir: i32,
    extend: bool,
    word: bool,
    line: bool,
  ) {
    let chars: Vec<char> = self.field_value(fid).chars().collect();
    let ed = self.editor_mut(fid);
    ed.cursor = if !word && !line && !extend && ed.cursor != ed.anchor {
      // A plain arrow over a selection collapses to that edge.
      if dir < 0 { ed.cursor.min(ed.anchor) } else { ed.cursor.max(ed.anchor) }
    } else {
      edit_target(&chars, ed.cursor, dir > 0, word, line)
    };
    if !extend {
      ed.anchor = ed.cursor;
    }
  }

  /// Move the caret to the start (`to_end = false`) or end of the field.
  pub(super) fn field_edge(
    &mut self,
    fid: FieldId,
    to_end: bool,
    extend: bool,
  ) {
    let len = self.field_value(fid).chars().count();
    let ed = self.editor_mut(fid);
    ed.cursor = if to_end { len } else { 0 };
    if !extend {
      ed.anchor = ed.cursor;
    }
  }
}

/// The caret target for a move / delete of the given granularity in the
/// `forward` direction: the field edge (`line`), the next word boundary
/// (`word`), or one character.
fn edit_target(
  chars: &[char],
  pos: usize,
  forward: bool,
  word: bool,
  line: bool,
) -> usize {
  if line {
    if forward { chars.len() } else { 0 }
  } else if word {
    if forward { word_end(chars, pos) } else { word_start(chars, pos) }
  } else if forward {
    (pos + 1).min(chars.len())
  } else {
    pos.saturating_sub(1)
  }
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}

/// Start of the word to the left of `pos` — skip non-word chars, then the word
/// (browser Option/Ctrl+Left / +Backspace).
fn word_start(chars: &[char], pos: usize) -> usize {
  let mut i = pos;
  while i > 0 && !is_word(chars[i - 1]) {
    i -= 1;
  }
  while i > 0 && is_word(chars[i - 1]) {
    i -= 1;
  }
  i
}

/// End of the word to the right of `pos`.
fn word_end(chars: &[char], pos: usize) -> usize {
  let mut i = pos;
  while i < chars.len() && !is_word(chars[i]) {
    i += 1;
  }
  while i < chars.len() && is_word(chars[i]) {
    i += 1;
  }
  i
}

#[cfg(test)]
mod tests {
  use super::{edit_target, word_end, word_start};

  fn chars(s: &str) -> Vec<char> {
    s.chars().collect()
  }

  #[test]
  fn word_nav_steps_over_punctuation() {
    // "http://10.121.121.166:3032" — word nav stops at alphanumeric runs,
    // skipping punctuation (like a browser's Option/Ctrl+arrow).
    let c = chars("http://10.121.121.166:3032");
    assert_eq!(word_start(&c, 26), 22); // start of "3032"
    assert_eq!(word_start(&c, 22), 18); // start of "166" (skips ':')
    assert_eq!(word_start(&c, 0), 0);
    assert_eq!(word_end(&c, 0), 4); // end of "http"
    assert_eq!(word_end(&c, 4), 9); // skip "://" then "10"
  }

  #[test]
  fn edit_target_by_granularity() {
    let c = chars("one two three");
    // Line: to the field edges.
    assert_eq!(edit_target(&c, 5, false, false, true), 0);
    assert_eq!(edit_target(&c, 5, true, false, true), 13);
    // Word: to the surrounding word boundaries.
    assert_eq!(edit_target(&c, 13, false, true, false), 8); // start of "three"
    assert_eq!(edit_target(&c, 0, true, true, false), 3); // end of "one"
    // Char: one step.
    assert_eq!(edit_target(&c, 5, false, false, false), 4);
    assert_eq!(edit_target(&c, 5, true, false, false), 6);
  }
}
