use super::Editor;
use crate::progress::save_progress_full;

/// Why a progress snapshot is being taken — governs whether an *unchanged*
/// position is pushed to the server, so leaving the reader can't clobber a
/// peer's newer position under last-write-wins.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SnapshotReason {
  /// Periodic save while reading: persist and push only what actually changed.
  Passive,
  /// Leaving the reader: persist the final state locally and flush reading
  /// time, but push the position only if the user moved since the last sync.
  Exit,
  /// Explicit user-initiated sync (`:sync`, sync-mode / auto-sync changes, the
  /// keep-local override): always push the current position.
  Explicit,
}

impl Editor {
  /// Width-independent reading percent (`0..=100`) of a document line: the
  /// shared non-whitespace-character fraction, so this reader, its peers, and
  /// the server all show the same percent for the same content whatever each
  /// one's wrap width (a line-index percent would disagree across widths).
  pub(crate) fn reading_percent(&self, line: usize) -> f64 {
    crate::word_anchor::fraction_of_line(&self.lines, &self.line_kinds, line)
      * 100.0
  }

  pub(crate) fn save_progress_snapshot(
    &mut self,
    reason: SnapshotReason,
  ) -> Result<(), Box<dyn std::error::Error>> {
    if self.total_lines == 0 {
      return Ok(());
    }
    // Reading progress is a property of the document buffer (index 0) alone.
    // While an overlay or split buffer owns the editor, `offset`/`cursor_y`/
    // `total_lines` describe *that* buffer — a save here would record e.g. the
    // top of a 3-line notification as "position 0 of a 3-line document" and
    // push it to the server with a fresh timestamp, clobbering the real
    // position on every device under last-write-wins. (This is exactly what
    // the offline "Sync failed" overlay used to do: its periodic passive
    // saves silently reset the book to 0%.) The document's own position was
    // saved when it was last active and is saved again on return/exit.
    if self.active_buffer != 0 {
      return Ok(());
    }
    if self.pdf_pending.is_some() && self.pdf_streaming.is_none() {
      return Ok(());
    }
    // A streaming-PDF resume is still pending: the cursor is parked on a
    // placeholder, not the saved row (which lands once its page streams in).
    // Don't overwrite the persisted position with the placeholder — keep the
    // real saved row until the restore target is applied.
    if self.pdf_restore_target.is_some() {
      return Ok(());
    }

    let current_line = self.offset + self.cursor_y;
    // A passive periodic save has nothing to do when neither the position nor
    // the viewport moved and no reading time accrued. Exit/Explicit always
    // persist the final state (so pure reading without cursor movement is still
    // recorded, and leaving captures the last position locally).
    if reason == SnapshotReason::Passive
      && current_line == self.last_offset
      && self.offset == self.last_saved_viewport_offset
      && !self.reading_dirty
    {
      return Ok(());
    }

    let (page, line_in_page) = match self.current_pdf_position() {
      Some((p, l)) => (Some(p), Some(l)),
      None => (None, None),
    };
    // Exact resume anchor origin: the page's first line for a streaming PDF
    // (page-local anchor), or the document start (line 0) for reflowable
    // formats (global anchor). A reflowable save has no `line_in_page`, so it
    // must anchor from 0 — subtracting nothing would make the range empty and
    // the anchor a useless 0.
    let word_start =
      line_in_page.map_or(0, |lip| current_line.saturating_sub(lip));
    let word_offset = Some(crate::word_anchor::words_in_range(
      &self.lines,
      &self.line_kinds,
      word_start,
      current_line,
    ));
    save_progress_full(
      self.document_hash,
      self.sync_now_ms(),
      current_line,
      self.total_lines,
      self.reading_percent(current_line),
      Some(self.offset),
      Some(self.cursor_y),
      page,
      line_in_page,
      word_offset,
      self.reading_time_seconds,
    )?;
    self.last_offset = current_line;
    self.last_saved_viewport_offset = self.offset;
    if self.snapshot_should_push(reason, current_line) {
      self.enqueue_progress_sync(current_line, page, line_in_page);
    }
    self.flush_reading_time();
    Ok(())
  }

  /// Whether a snapshot for `reason` should push the current position to the
  /// server. Always on an explicit sync; otherwise only when the position moved
  /// since the last sync. This is what stops leaving the reader (or a passive
  /// save) from re-asserting an *unchanged* position with a fresh timestamp and
  /// clobbering a peer's newer one under last-write-wins — the CLI analogue of
  /// the PWA back-button regression. The changed position was already pushed
  /// when it changed, so nothing is lost.
  pub(crate) fn snapshot_should_push(
    &self,
    reason: SnapshotReason,
    current_line: usize,
  ) -> bool {
    reason == SnapshotReason::Explicit
      || self.last_synced_offset != Some(current_line)
  }

  /// Accrue active reading time for the current tick. Time only counts while
  /// the user has been active within `READING_IDLE_SECONDS`; a long idle gap
  /// (or a suspended process) naturally stops accrual because `last_activity`
  /// goes stale. Whole seconds roll into `reading_time_seconds`; a fractional
  /// carry keeps short polls from being lost.
  pub(crate) fn accrue_reading_time(&mut self) {
    let now = std::time::Instant::now();
    let delta = now.saturating_duration_since(self.reading_last_tick);
    self.reading_last_tick = now;
    // Don't count the PDF loading splash (one-line placeholder buffer) as
    // reading time.
    if self.pdf_pending.is_some() && self.pdf_streaming.is_none() {
      return;
    }
    if now.duration_since(self.last_activity).as_secs()
      >= crate::reading_stats::READING_IDLE_SECONDS
    {
      return;
    }
    self.reading_accrued += delta.as_secs_f64();
    let whole = self.reading_accrued.floor();
    if whole >= 1.0 {
      self.reading_accrued -= whole;
      self.reading_time_seconds =
        self.reading_time_seconds.saturating_add(whole as u64);
      self.reading_dirty = true;
    }
  }

  /// Persist newly-accrued reading time to the per-day local store and queue it
  /// for the server (per-book cumulative + per-day cumulative). The per-book
  /// total is written by `save_progress_full` alongside the position.
  pub(crate) fn flush_reading_time(&mut self) {
    self.reading_dirty = false;
    self.reading_last_flush = std::time::Instant::now();
    let delta =
      self.reading_time_seconds.saturating_sub(self.reading_persisted_seconds);
    if delta == 0 {
      return;
    }
    self.reading_persisted_seconds = self.reading_time_seconds;
    let day_total = crate::reading_stats::add_today_seconds(delta);
    self.enqueue_reading_time_sync(self.reading_time_seconds);
    self.enqueue_reading_day_sync(crate::reading_stats::today_key(), day_total);
  }

  /// While purely reading (no cursor movement), persist accrued reading time on
  /// a slow cadence so a crash loses at most a few seconds. Called each tick.
  pub(crate) fn maybe_flush_reading_time(&mut self) {
    if self.reading_dirty
      && self.reading_last_flush.elapsed().as_secs() >= 30
      && let Err(e) = self.save_progress_snapshot(SnapshotReason::Passive)
    {
      self.debug_log_error(&format!("periodic reading-time flush failed: {e}"));
    }
  }
}
