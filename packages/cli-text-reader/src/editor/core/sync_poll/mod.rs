//! Inbound side of the editor's sync glue: drain engine notifications once per
//! render tick and apply them to the live editor. All of this is a no-op when
//! `self.sync` is `None`, so the offline reader pays nothing. The outbound
//! enqueue helpers live in `sync_enqueue`; annotation application in
//! `sync_apply`.

use super::Editor;
use crate::sync::SyncEvent;

mod grace;
mod jump;

impl Editor {
  /// Drain engine notifications (called once per render tick). Progress raises
  /// the jump prompt; annotations for the current book are applied + persisted.
  pub(crate) fn poll_sync(&mut self) {
    let Some(sync) = self.sync.as_ref() else {
      return;
    };
    let events = sync.drain();
    if events.is_empty() {
      return;
    }
    let current_book = self.book_id.clone();
    let is_current = |book_id: &str| current_book.as_deref() == Some(book_id);
    for event in events {
      match event {
        SyncEvent::Status { ok, message } => {
          let mut lines = vec![format!("  {message}")];
          if !ok {
            lines.push("  Check :connect URL, device token, tier/access, and server logs.".to_string());
          }
          lines.push("  :q to dismiss".to_string());
          self.create_overlay("notification", lines);
          self.mark_dirty();
        }
        SyncEvent::Progress(progress) if is_current(&progress.book_id) => {
          self.handle_server_progress(progress);
        }
        SyncEvent::SyncCycleComplete => {
          self.startup_progress_reconcile = false;
        }
        SyncEvent::Bookmark(bookmark) if is_current(&bookmark.book_id) => {
          self.apply_remote_bookmark(&bookmark);
        }
        SyncEvent::Highlight(highlight) if is_current(&highlight.book_id) => {
          self.apply_remote_highlight(&highlight);
        }
        SyncEvent::Note(note) if is_current(&note.book_id) => {
          self.apply_remote_note(&note);
        }
        _ => {}
      }
    }
  }

  fn handle_server_progress(&mut self, progress: crate::sync::ServerProgress) {
    // An explicit `:server-progress` always goes to the server's position for
    // this book, whatever it is; the automatic path only surfaces a genuinely
    // newer, different position as a prompt.
    let requested = self.server_progress_jump_requested_at.is_some();
    if !requested
      && (self.server_progress_matches_local_position(&progress)
        || !self.server_progress_is_newer_than_local(&progress))
    {
      self.clear_server_progress_prompt();
      if progress.updated_at > 0 {
        self.last_local_progress_updated_at = Some(
          self
            .last_local_progress_updated_at
            .map_or(progress.updated_at, |local| {
              local.max(progress.updated_at)
            }),
        );
      }
      return;
    }

    self.pending_server_progress = Some(progress);
    // A genuinely new server position restarts the decision: cancel any
    // in-progress "keep local" grace so a stale countdown can't clear the fresh
    // prompt (the reader re-arms it by scrolling again).
    self.server_progress_scroll_at = None;

    // While the PDF is still opening, the flat buffer is only the one-line
    // splash and there is no page table to anchor against — applying a jump now
    // would land on the wrong row once content fills in (the bug this guards,
    // visible in slow debug builds). Defer: remember whether this should
    // auto-apply (the startup window may close before a slow open finishes) and
    // resolve once the stream installs. Latch the flag: a slow PDF open keeps
    // re-delivering this row on later poll cycles, by which point the one-shot
    // `startup_progress_reconcile` has cleared — overwriting (rather than
    // OR-ing) would silently downgrade the pending auto-apply to a prompt.
    if self.pdf_pending.is_some() && self.pdf_streaming.is_none() {
      self.pending_server_progress_autoapply |=
        self.startup_progress_reconcile || requested;
      self.server_progress_jump_requested_at = None;
      return;
    }

    if self.startup_progress_reconcile || requested {
      self.finish_server_progress_request();
      self.jump_to_server_progress();
      return;
    }
    self.server_progress_prompt = true;
    self.mark_dirty();
  }

  /// Apply (or surface) a server-progress jump that was deferred while the PDF
  /// was still opening. Called right after the streaming page table installs,
  /// when a stable (page, line_in_page) anchor finally exists.
  pub(crate) fn resolve_pending_server_progress_after_install(&mut self) {
    if self.pending_server_progress.is_none() {
      return;
    }
    if self.pending_server_progress_autoapply {
      self.pending_server_progress_autoapply = false;
      self.jump_to_server_progress();
    } else {
      self.server_progress_prompt = true;
      self.mark_dirty();
    }
  }

  fn server_progress_matches_local_position(
    &self,
    progress: &crate::sync::ServerProgress,
  ) -> bool {
    // Streaming PDF: the authoritative comparison is (page, line_in_page). Flat
    // offsets don't agree until every page is loaded, so comparing them while
    // streaming would spuriously report "different" and trigger a jump.
    if let (Some((local_page, local_line_in_page)), Some(page), Some(line)) =
      (self.current_pdf_position(), progress.page, progress.line_in_page)
    {
      return local_page == page && local_line_in_page == line;
    }
    let local_line = self.offset + self.cursor_y;
    if progress.offset == local_line {
      return true;
    }
    if let (Some(viewport_offset), Some(cursor_y)) =
      (progress.viewport_offset, progress.cursor_y)
    {
      return viewport_offset == self.offset && cursor_y == self.cursor_y
        || viewport_offset.saturating_add(cursor_y) == local_line;
    }
    false
  }

  fn server_progress_is_newer_than_local(
    &self,
    progress: &crate::sync::ServerProgress,
  ) -> bool {
    let Some(local_updated_at) = self.last_local_progress_updated_at else {
      return true;
    };
    progress.updated_at > local_updated_at
  }

  fn clear_server_progress_prompt(&mut self) {
    if self.server_progress_prompt || self.pending_server_progress.is_some() {
      self.server_progress_prompt = false;
      self.server_progress_scroll_at = None;
      self.pending_server_progress = None;
      self.mark_dirty();
    }
  }

  /// Commit a resolved server position to the viewport and adopt its timestamp
  /// as the new local baseline (so the next reconcile compares against it).
  fn commit_server_position(
    &mut self,
    offset: usize,
    cursor_y: usize,
    updated_at: i64,
  ) {
    self.offset = offset;
    self.cursor_y = cursor_y;
    self.last_offset = self.offset + self.cursor_y;
    self.last_saved_viewport_offset = self.offset;
    self.last_synced_offset = Some(self.last_offset);
    if updated_at > 0 {
      self.last_local_progress_updated_at = Some(updated_at);
    }
    self.mark_dirty();
  }

  /// Flat line for a percentage position in a streaming PDF. Maps the fraction
  /// onto this reader's own line count (the space `percentage` was measured in
  /// on the sending device too), resolves which page that line lands on, and
  /// returns a page-anchored flat line so it stays valid while pages stream in.
  /// `None` when not streaming a PDF or percentage is 0.
  ///
  /// Mapping onto the *page* count instead lands proportionally by page number,
  /// which is wildly off when pages differ in height — a full-page cover image
  /// inflates the early pages, so a PWA-synced 24% landed near 35%.
  fn pdf_line_for_percent(&self, percentage: f64) -> Option<usize> {
    let state = self.pdf_streaming.as_ref()?;
    if state.pages.is_empty() || percentage <= 0.0 || self.total_lines == 0 {
      return None;
    }
    // `percentage` is the width-independent character fraction, so resolve it
    // to a line through the character anchor (not a flat line-fraction,
    // which is wildly off when pages differ in height — a full-page cover
    // inflates the early pages).
    let target_line = crate::word_anchor::line_for_fraction(
      &self.lines,
      &self.line_kinds,
      percentage / 100.0,
    )
    .min(self.total_lines.saturating_sub(1));
    let (page_index, line_in_page) =
      super::page_and_offset_for_line(state, target_line);
    self.pdf_line_for_page_position((page_index + 1) as u32, line_in_page)
  }

  /// Target line for the flat (non-PDF) fallback: the saved offset when the
  /// line-spaces agree, otherwise the percentage mapped onto the local length.
  fn server_target_line(
    &self,
    progress: &crate::sync::ServerProgress,
  ) -> usize {
    if progress.total_lines == self.total_lines || progress.percentage <= 0.0 {
      progress.offset
    } else {
      // Cross-width: map the shared character fraction onto this reader's
      // lines.
      crate::word_anchor::line_for_fraction(
        &self.lines,
        &self.line_kinds,
        progress.percentage / 100.0,
      )
    }
  }
}

#[cfg(test)]
mod tests_basic;
#[cfg(test)]
mod tests_pdf;
