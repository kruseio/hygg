//! Unit tests for the inbound sync glue in `sync_poll.rs`. Split out to keep
//! the production module within the repository's per-file line budget.

use super::*;

fn progress(offset: usize, updated_at: i64) -> crate::sync::ServerProgress {
  crate::sync::ServerProgress {
    book_id: "doc".to_string(),
    offset,
    total_lines: 0,
    percentage: 0.0,
    viewport_offset: None,
    cursor_y: None,
    page: None,
    line_in_page: None,
    word_offset: None,
    updated_at,
  }
}

fn editor_at(line: usize, updated_at: i64) -> Editor {
  let mut editor = Editor::new(vec!["line".to_string(); 100], 80);
  editor.offset = line;
  editor.cursor_y = 0;
  editor.last_local_progress_updated_at = Some(updated_at);
  editor
}

#[test]
fn stale_server_progress_does_not_prompt() {
  let mut editor = editor_at(40, 2_000);
  editor.handle_server_progress(progress(10, 1_000));

  assert!(!editor.server_progress_prompt);
  assert!(editor.pending_server_progress.is_none());
}

#[test]
fn equal_server_position_does_not_prompt_even_when_newer() {
  let mut editor = editor_at(40, 1_000);
  editor.handle_server_progress(progress(40, 2_000));

  assert!(!editor.server_progress_prompt);
  assert!(editor.pending_server_progress.is_none());
  assert_eq!(editor.last_local_progress_updated_at, Some(2_000));
}

#[test]
fn newer_different_server_position_prompts() {
  let mut editor = editor_at(40, 1_000);
  editor.handle_server_progress(progress(75, 2_000));

  assert!(editor.server_progress_prompt);
  assert_eq!(
    editor.pending_server_progress.as_ref().map(|p| p.offset),
    Some(75)
  );
}

#[test]
fn startup_newer_server_progress_auto_applies_without_prompt() {
  let mut editor = editor_at(40, 1_000);
  editor.startup_progress_reconcile = true;
  editor.handle_server_progress(progress(75, 2_000));

  assert!(!editor.server_progress_prompt);
  assert!(editor.pending_server_progress.is_none());
  assert_eq!(editor.last_offset, 75);
  assert_eq!(editor.last_local_progress_updated_at, Some(2_000));
}

#[test]
fn cross_paginated_server_progress_maps_by_percentage() {
  // A position synced from a reader that paginates the document differently
  // (here 2653 lines vs this editor's 100) is re-mapped onto this reader's own
  // length by percentage — not applied as the raw, out-of-range line index.
  let mut editor = editor_at(0, 1_000);
  editor.startup_progress_reconcile = true;
  let mut p = progress(605, 2_000);
  p.total_lines = 2653;
  p.percentage = 50.0;
  editor.handle_server_progress(p);

  // 50% of 100 lines = line 50, not the raw offset 605.
  assert_eq!(editor.last_offset, 50);
  assert_eq!(editor.last_local_progress_updated_at, Some(2_000));
}

#[test]
fn startup_older_server_progress_does_not_apply_when_local_seeded() {
  // Regression: resuming a PDF used to leave `last_local_progress_updated_at`
  // None, so an *older* server position looked "newer" and the startup
  // reconcile jumped to it. With the local timestamp seeded from the restored
  // position, a staler server row must be ignored even during the auto-apply
  // window.
  let mut editor = editor_at(10, 2_000);
  editor.startup_progress_reconcile = true;
  editor.handle_server_progress(progress(2_311, 1_000));

  assert!(!editor.server_progress_prompt);
  assert!(editor.pending_server_progress.is_none());
  assert_eq!(editor.offset, 10);
}

#[test]
fn startup_reconcile_skips_push_while_pdf_is_pending() {
  // The one-line splash buffer (total_lines == 1) must not be uploaded as a
  // real position. With the local timestamp now seeded, the pdf-pending guard
  // is what prevents pushing `offset + cursor_y` of the splash.
  let mut editor = Editor::new(vec![String::new()], 80);
  editor.book_id = Some("doc".to_string());
  editor.last_local_progress_updated_at = Some(1_000);
  editor.pdf_pending = Some(crate::editor::streaming::PendingPdfStream {
    receiver: std::sync::mpsc::channel().1,
    started_at: std::time::Instant::now(),
    canonical_path_display: String::new(),
    restore_line_in_page: None,
    restore_cursor_y: None,
    restore_word_offset: None,
  });

  editor.queue_reconcile_sync_state(true);

  assert!(editor.startup_progress_reconcile);
  assert_eq!(editor.last_synced_offset, None);
}

#[test]
fn after_startup_window_newer_server_progress_prompts() {
  let mut editor = editor_at(40, 1_000);
  editor.startup_progress_reconcile = true;
  editor.handle_server_progress(progress(75, 2_000));

  editor.startup_progress_reconcile = false;
  editor.handle_server_progress(progress(90, 3_000));

  assert!(editor.server_progress_prompt);
  assert_eq!(
    editor.pending_server_progress.as_ref().map(|p| p.offset),
    Some(90)
  );
}

fn pending_pdf_splash() -> crate::editor::streaming::PendingPdfStream {
  crate::editor::streaming::PendingPdfStream {
    receiver: std::sync::mpsc::channel().1,
    started_at: std::time::Instant::now(),
    canonical_path_display: String::new(),
    restore_line_in_page: None,
    restore_cursor_y: None,
    restore_word_offset: None,
  }
}

#[test]
fn server_progress_while_pdf_opening_is_deferred_not_applied() {
  // The flat splash buffer has no page table; applying a jump now would land
  // on the wrong row once content fills in. The position must be stashed (with
  // its auto-apply intent) and neither move the cursor nor raise the prompt.
  let mut editor = Editor::new(vec![String::new()], 80);
  editor.last_local_progress_updated_at = Some(1_000);
  editor.startup_progress_reconcile = true;
  editor.offset = 0;
  editor.cursor_y = 0;
  editor.pdf_pending = Some(pending_pdf_splash());

  let mut p = progress(2_311, 2_000);
  p.page = Some(8);
  p.line_in_page = Some(3);
  editor.handle_server_progress(p);

  assert_eq!((editor.offset, editor.cursor_y), (0, 0));
  assert!(!editor.server_progress_prompt);
  assert!(editor.pending_server_progress.is_some());
  assert!(editor.pending_server_progress_autoapply);
}

#[test]
fn deferred_server_progress_prompts_when_not_auto_applying() {
  // Deferred outside the startup window: once a real buffer exists the
  // position should surface as a prompt, not silently jump.
  let mut editor = Editor::new(vec!["line".to_string(); 100], 80);
  editor.pending_server_progress = Some(progress(50, 2_000));
  editor.pending_server_progress_autoapply = false;

  editor.resolve_pending_server_progress_after_install();

  assert!(editor.server_progress_prompt);
  assert!(editor.pending_server_progress.is_some());
}

#[test]
fn scrolling_arms_the_grace_only_while_the_prompt_is_up() {
  let mut editor = editor_at(40, 1_000);
  // No prompt: a move must not arm the grace.
  editor.note_user_scrolled();
  assert!(editor.server_progress_scroll_at.is_none());

  editor.handle_server_progress(progress(75, 2_000));
  assert!(editor.server_progress_prompt);

  // First move while the prompt is up arms the grace...
  editor.note_user_scrolled();
  let armed = editor.server_progress_scroll_at;
  assert!(armed.is_some());
  // ...and a later move does not reset it (the countdown runs from the first
  // scroll, so the prompt reliably clears ~3s after scrolling begins).
  editor.note_user_scrolled();
  assert_eq!(editor.server_progress_scroll_at, armed);
}

#[test]
fn explicit_request_jumps_even_when_the_server_is_not_newer() {
  // `:server-progress` arms `server_progress_jump_requested_at`; the delivered
  // position then jumps even though it is older than local (the automatic path
  // would skip it as "not newer"). This is what makes the command reliably go
  // to the server position instead of needing a second invocation.
  let mut editor = editor_at(10, 5_000);
  editor.server_progress_jump_requested_at = Some(std::time::Instant::now());

  editor.handle_server_progress(progress(80, 2_000));

  assert_eq!(editor.last_offset, 80, "jumped to the server position");
  assert!(editor.server_progress_jump_requested_at.is_none());
  assert!(!editor.server_progress_prompt);
  assert!(editor.pending_server_progress.is_none());
}

#[test]
fn explicit_request_times_out_when_nothing_is_delivered() {
  // If the re-fetch returns no position for this book, the armed request
  // expires instead of hanging (the overlay note is only shown when one is
  // up).
  let mut editor = editor_at(10, 1_000);
  editor.server_progress_jump_requested_at =
    std::time::Instant::now().checked_sub(std::time::Duration::from_secs(10));
  assert!(editor.server_progress_jump_requested_at.is_some());

  editor.tick_server_progress_grace();

  assert!(editor.server_progress_jump_requested_at.is_none());
}

#[test]
fn grace_expiry_keeps_local_and_clears_the_prompt() {
  let mut editor = editor_at(40, 1_000);
  editor.handle_server_progress(progress(75, 2_000));
  assert!(editor.server_progress_prompt);
  editor.note_user_scrolled();

  // Pretend the grace has already elapsed since the reader scrolled away, and
  // keep the persist path off the filesystem for this headless test.
  editor.total_lines = 0;
  editor.server_progress_scroll_at =
    std::time::Instant::now().checked_sub(std::time::Duration::from_secs(5));
  assert!(editor.server_progress_scroll_at.is_some());

  editor.tick_server_progress_grace();

  // The server jump is dropped and the prompt is gone — the local position
  // wins, exactly like an explicit `:local-progress`.
  assert!(!editor.server_progress_prompt);
  assert!(editor.pending_server_progress.is_none());
  assert!(editor.server_progress_scroll_at.is_none());
}
