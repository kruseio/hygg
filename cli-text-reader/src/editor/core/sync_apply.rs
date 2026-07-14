//! Applies bookmark/highlight/note changes pulled from the server to the live
//! editor state and persists them locally. These run only for the currently
//! open book (`poll_sync` filters by `book_id`) and never re-enqueue — applying
//! a change writes in-memory state + disk directly, so there is no echo loop
//! (re-applying one's own change is idempotent).

use super::Editor;
use crate::notes::{Note, save_notes};
use crate::sync::{ServerBookmark, ServerHighlight, ServerNote};

impl Editor {
  pub(crate) fn apply_remote_bookmark(&mut self, bookmark: &ServerBookmark) {
    let Some(mark) = bookmark.mark.chars().next() else {
      return;
    };
    let changed = if bookmark.deleted {
      self.marks.remove(&mark).is_some()
    } else {
      let pos = (bookmark.line, bookmark.col);
      if self.marks.get(&mark) == Some(&pos) {
        false
      } else {
        self.marks.insert(mark, pos);
        true
      }
    };
    if changed {
      self.save_bookmarks();
      self.mark_dirty();
    }
  }

  pub(crate) fn apply_remote_highlight(&mut self, highlight: &ServerHighlight) {
    let matches = |h: &crate::highlights::Highlight| {
      h.start == highlight.start && h.end == highlight.end
    };
    let changed = if highlight.deleted {
      let before = self.highlights.highlights.len();
      self.highlights.highlights.retain(|h| !matches(h));
      self.highlights.highlights.len() != before
    } else {
      // add_highlight dedups on (start, end) and reports whether it inserted.
      self.highlights.add_highlight(highlight.start, highlight.end)
    };
    if changed {
      self.save_highlights();
      self.mark_dirty();
    }
  }

  pub(crate) fn apply_remote_note(&mut self, note: &ServerNote) {
    let existing = self.notes.notes.iter().position(|n| n.id == note.id);
    let changed = match (note.deleted, existing) {
      (true, Some(idx)) => {
        self.notes.notes.remove(idx);
        true
      }
      (true, None) => false,
      (false, Some(idx)) => {
        let local = &mut self.notes.notes[idx];
        if local.body == note.body && local.line == note.line {
          false
        } else {
          local.body = note.body.clone();
          local.line = note.line;
          local.updated_at = note.updated_at;
          true
        }
      }
      (false, None) => {
        self.notes.notes.push(Note {
          id: note.id.clone(),
          body: note.body.clone(),
          line: note.line,
          created_at: note.created_at,
          updated_at: note.updated_at,
        });
        true
      }
    };
    if changed {
      if let Err(e) = save_notes(self.document_hash, &self.notes) {
        self.debug_log_error(&format!("Failed to save synced notes: {e}"));
      }
      self.mark_dirty();
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn editor() -> Editor {
    Editor::new(vec!["line one".to_string(), "line two".to_string()], 80)
  }

  fn bookmark(mark: &str, line: usize, deleted: bool) -> ServerBookmark {
    ServerBookmark {
      book_id: "b".into(),
      mark: mark.into(),
      line,
      col: 0,
      deleted,
    }
  }

  fn note(id: &str, body: &str, updated_at: i64, deleted: bool) -> ServerNote {
    ServerNote {
      book_id: "b".into(),
      id: id.into(),
      body: body.into(),
      line: Some(3),
      created_at: 1,
      updated_at,
      deleted,
    }
  }

  #[test]
  fn remote_bookmark_is_applied_then_removed() {
    let mut e = editor();
    e.apply_remote_bookmark(&bookmark("a", 5, false));
    assert_eq!(e.marks.get(&'a'), Some(&(5, 0)));
    e.apply_remote_bookmark(&bookmark("a", 0, true));
    assert!(!e.marks.contains_key(&'a'));
  }

  #[test]
  fn remote_highlight_is_idempotent_then_tombstoned() {
    let mut e = editor();
    let add = ServerHighlight {
      book_id: "b".into(),
      start: 10,
      end: 20,
      deleted: false,
    };
    e.apply_remote_highlight(&add);
    e.apply_remote_highlight(&add); // re-applying must not duplicate
    assert_eq!(e.highlights.highlights.len(), 1);
    let del = ServerHighlight {
      book_id: "b".into(),
      start: 10,
      end: 20,
      deleted: true,
    };
    e.apply_remote_highlight(&del);
    assert!(e.highlights.highlights.is_empty());
  }

  #[test]
  fn remote_note_is_inserted_edited_then_deleted() {
    let mut e = editor();
    e.apply_remote_note(&note("n1", "hello", 1, false));
    assert_eq!(e.notes.notes.len(), 1);
    assert_eq!(e.notes.notes[0].body, "hello");

    e.apply_remote_note(&note("n1", "edited", 2, false));
    assert_eq!(e.notes.notes.len(), 1, "same id edits in place");
    assert_eq!(e.notes.notes[0].body, "edited");

    e.apply_remote_note(&note("n1", "", 3, true));
    assert!(e.notes.notes.is_empty());
  }
}
