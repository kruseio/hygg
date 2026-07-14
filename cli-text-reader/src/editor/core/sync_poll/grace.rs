//! The jump-or-keep-local grace around the "position changed on server" prompt,
//! and the explicit `:server-progress` re-fetch request. Split out of
//! `sync_poll` to keep each file within the repository's per-file line budget;
//! behaviour is unchanged.

use super::Editor;

/// How long the "position changed on server" prompt lingers after the reader
/// starts scrolling before it clears and the local position wins. Matches the
/// PWA toast so the jump/keep-local UX is identical across clients.
const SERVER_PROGRESS_GRACE: std::time::Duration =
  std::time::Duration::from_secs(3);

/// How long an explicit `:server-progress` waits for its re-fetch to deliver a
/// position before giving up and reporting that the server has none.
const SERVER_PROGRESS_REQUEST_TIMEOUT: std::time::Duration =
  std::time::Duration::from_secs(6);

impl Editor {
  /// A deliberate move while the "position changed on server" prompt is up
  /// means the reader is keeping their own place. Start the grace countdown
  /// (once, on the first move, so it runs from when scrolling began);
  /// `:server-progress` can still jump until it elapses.
  pub(crate) fn note_user_scrolled(&mut self) {
    if self.server_progress_prompt && self.server_progress_scroll_at.is_none() {
      self.server_progress_scroll_at = Some(std::time::Instant::now());
    }
  }

  /// Complete an explicit `:server-progress`: drop the request and dismiss the
  /// "checking…" overlay so the jump it triggers is actually visible.
  pub(crate) fn finish_server_progress_request(&mut self) {
    self.server_progress_jump_requested_at = None;
    if self.view_mode == crate::core_types::ViewMode::Overlay {
      self.close_overlay();
    }
  }

  /// Per-render-tick housekeeping for the server-progress prompt: expire the
  /// post-scroll grace, and time out an explicit `:server-progress` re-fetch
  /// that never delivered. Both are cheap `Option` checks — no-ops until armed.
  pub(crate) fn tick_server_progress_grace(&mut self) {
    self.tick_scroll_grace();
    self.tick_server_progress_request();
  }

  /// If an explicit `:server-progress` re-fetch hasn't delivered a position in
  /// time (the server has none for this book, or the network failed), stop
  /// waiting and turn the "checking…" overlay into a clear note.
  fn tick_server_progress_request(&mut self) {
    let Some(at) = self.server_progress_jump_requested_at else {
      return;
    };
    if at.elapsed() < SERVER_PROGRESS_REQUEST_TIMEOUT {
      return;
    }
    self.server_progress_jump_requested_at = None;
    if self.view_mode == crate::core_types::ViewMode::Overlay {
      self.create_overlay(
        "notification",
        vec![
          "  No saved position on the server for this document.".to_string(),
          "  :q to dismiss".to_string(),
        ],
      );
      self.mark_dirty();
    }
  }

  /// Once the post-scroll grace elapses, clear the prompt and keep the local
  /// position over the server's — the same "keep local" effect as
  /// `:local-progress`, so peers adopt where this reader actually is.
  fn tick_scroll_grace(&mut self) {
    let Some(at) = self.server_progress_scroll_at else {
      return;
    };
    if at.elapsed() < SERVER_PROGRESS_GRACE {
      return;
    }
    self.server_progress_scroll_at = None;
    // Keeping local also cancels any explicit `:server-progress` re-fetch, so
    // it can't jump us away after we've chosen our own place.
    self.server_progress_jump_requested_at = None;
    if self.book_id.is_some() {
      self.set_auto_sync_optin(true);
    }
    let overwrite_after =
      self.pending_server_progress.as_ref().map(|progress| progress.updated_at);
    self.pending_server_progress = None;
    self.server_progress_prompt = false;
    self.startup_progress_reconcile = false;
    let _ = self.queue_current_sync_state_newer_than(overwrite_after);
    self.mark_dirty();
  }
}
