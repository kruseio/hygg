//! Unit tests for the progress-snapshot guards (`progress_snapshot.rs`) that
//! keep local saves and server pushes honest. Split out of `tests_basic.rs`
//! to keep each file within the repository's per-file line budget.

use super::Editor;
use crate::editor::core::SnapshotReason;

fn editor_at(line: usize, updated_at: i64) -> Editor {
  let mut editor = Editor::new(vec!["line".to_string(); 100], 80);
  editor.offset = line;
  editor.cursor_y = 0;
  editor.last_local_progress_updated_at = Some(updated_at);
  editor
}

#[test]
fn snapshot_skips_while_a_non_document_buffer_is_active() {
  // Regression: while the "Sync failed" notification overlay was up (server
  // unreachable), periodic passive saves recorded the *overlay's* position —
  // line 0 of a 3-line buffer — as reading progress, resetting the book to 0%
  // locally and, with a fresh timestamp, on every peer once the server came
  // back (last-write-wins). Progress must only ever be snapshotted from the
  // document buffer.
  let mut editor = editor_at(40, 1_000);
  editor.last_offset = 40;
  editor.create_overlay(
    "notification",
    vec!["  Sync failed".to_string(), "  :q to dismiss".to_string()],
  );
  assert_eq!(editor.active_buffer, 1);

  editor.save_progress_snapshot(SnapshotReason::Passive).unwrap();

  assert_eq!(editor.last_offset, 40, "overlay position must not be recorded");
  assert_eq!(editor.last_synced_offset, None, "and must never be pushed");
}

#[test]
fn exit_snapshot_does_not_re_push_an_unchanged_position() {
  let mut editor = editor_at(40, 1_000);
  // Line 40 was already synced this session.
  editor.last_synced_offset = Some(40);

  // Leaving without moving must not re-assert it — that fresh-timestamped push
  // would clobber a peer's newer position under last-write-wins.
  assert!(!editor.snapshot_should_push(SnapshotReason::Exit, 40));
  assert!(!editor.snapshot_should_push(SnapshotReason::Passive, 40));

  // But an explicit `:sync` always pushes, and a position the user *moved* to
  // still flushes on exit.
  assert!(editor.snapshot_should_push(SnapshotReason::Explicit, 40));
  assert!(editor.snapshot_should_push(SnapshotReason::Exit, 55));
}
