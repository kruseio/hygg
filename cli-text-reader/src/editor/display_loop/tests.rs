use super::super::core::Editor;
use super::{
  FAST_EVENT_POLL_MS, PDF_LOADING_EVENT_POLL_MS, event_poll_timeout,
};
use crate::editor::streaming::{PendingPdfStream, StreamReady};
use std::sync::mpsc;
use std::time::Instant;

fn test_editor() -> Editor {
  let mut editor = Editor::new(vec!["line".to_string()], 80);
  editor.height = 24;
  editor.width = 80;
  editor
}

fn rendered(buffer: Vec<u8>) -> String {
  String::from_utf8(buffer).expect("cursor commands should be utf8")
}

#[test]
fn idle_cursor_show_marks_cursor_visible() {
  let mut editor = test_editor();
  editor.show_cursor = true;
  editor.cursor_currently_visible = false;

  let mut buffer = Vec::new();
  editor.show_idle_cursor_if_needed(&mut buffer).unwrap();

  assert!(rendered(buffer).contains("\x1b[?25h"));
  assert!(editor.cursor_currently_visible);
}

#[test]
fn idle_cursor_show_skips_redundant_show_when_already_visible() {
  let mut editor = test_editor();
  editor.show_cursor = true;
  editor.cursor_currently_visible = true;

  let mut buffer = Vec::new();
  editor.show_idle_cursor_if_needed(&mut buffer).unwrap();

  assert!(buffer.is_empty());
  assert!(editor.cursor_currently_visible);
}

#[test]
fn idle_cursor_show_skips_show_while_pdf_is_pending() {
  let mut editor = test_editor();
  editor.show_cursor = true;
  editor.cursor_currently_visible = false;
  let (_tx, rx) = mpsc::channel::<StreamReady>();
  editor.pdf_pending = Some(PendingPdfStream {
    receiver: rx,
    started_at: Instant::now(),
    canonical_path_display: "pending.pdf".to_string(),
    restore_line_in_page: None,
    restore_cursor_y: None,
  });

  let mut buffer = Vec::new();
  editor.show_idle_cursor_if_needed(&mut buffer).unwrap();

  assert!(buffer.is_empty());
  assert!(!editor.cursor_currently_visible);
}

#[test]
fn event_poll_timeout_uses_spinner_cadence_for_pdf_loading() {
  assert_eq!(
    event_poll_timeout(false, false, true, false, false),
    std::time::Duration::from_millis(PDF_LOADING_EVENT_POLL_MS)
  );
  assert_eq!(
    event_poll_timeout(false, false, false, true, false),
    std::time::Duration::from_millis(PDF_LOADING_EVENT_POLL_MS)
  );
}

#[test]
fn event_poll_timeout_keeps_fast_cadence_for_redraws_and_demo() {
  assert_eq!(
    event_poll_timeout(true, false, true, false, false),
    std::time::Duration::from_millis(FAST_EVENT_POLL_MS)
  );
  assert_eq!(
    event_poll_timeout(false, true, false, false, false),
    std::time::Duration::from_millis(FAST_EVENT_POLL_MS)
  );
}
