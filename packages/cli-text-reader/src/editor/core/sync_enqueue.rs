//! Outbound side of the editor's sync glue: queue local metadata, reading
//! position, reading time and annotations for upload. All of this is a no-op
//! when `self.sync` is `None`, so the offline reader pays nothing. Each method
//! builds a payload and does one non-blocking channel send. Mirrors the
//! non-blocking poll pattern used by `pdf_poll` and the TTS worker.

use chrono::Utc;
use uuid::Uuid;

use super::Editor;
use crate::sync::{AnnotationOp, BookUpload, ProgressPayload};

impl Editor {
  /// Now, in the server's clock domain: this device's wall clock corrected by
  /// the skew offset the sync engine learns on every pull, so a timestamp this
  /// reader stamps on an op orders correctly against peers under
  /// last-write-wins. Falls back to the raw local clock when sync isn't running
  /// (offline — there is no peer to compare against yet).
  pub(crate) fn sync_now_ms(&self) -> i64 {
    let local = Utc::now().timestamp_millis();
    self.sync.as_ref().map_or(local, |s| s.corrected(local))
  }

  /// PDF page count for the book heuristic, once the stream is available.
  fn pdf_page_count(&self) -> Option<u32> {
    self.pdf_streaming.as_ref().map(|s| s.pages.len() as u32)
  }

  /// Whether the current document looks like a book (format + length signals).
  fn looks_like_book(&self) -> bool {
    let format = self
      .source_path
      .as_deref()
      .or(self.pdf_source_path.as_deref())
      .map(crate::library::kind_from_path)
      .unwrap_or_else(|| "text".to_string());
    hygg_shared::sync::looks_like_book(
      &format,
      self.total_lines,
      self.pdf_page_count(),
    )
  }

  /// Whether the current document should sync *automatically* — the account
  /// scope combined with this device's opt-in and the book heuristic. Gates the
  /// automatic enqueue helpers below; explicit `:sync` opts the document in
  /// first so it always flushes. The master switch and `SyncMode` are applied
  /// separately (engine spawn, `syncs_state`/`syncs_blob`).
  pub(crate) fn doc_auto_syncs(&self) -> bool {
    hygg_shared::sync::should_auto_sync(
      self.sync_policy,
      self.auto_sync_optin,
      self.looks_like_book(),
    )
  }
  /// Queue the current document's metadata + bytes for upload. The engine reads
  /// bytes on its own thread; this method only sends the source path.
  pub(crate) fn enqueue_current_book_sync(&self) {
    let (Some(sync), Some(book_id)) =
      (self.sync.as_ref(), self.book_id.as_ref())
    else {
      return;
    };
    // `off` syncs nothing for this document — not even the metadata record.
    if !self.sync_mode.syncs_state() || !self.doc_auto_syncs() {
      return;
    }
    let Some(path) =
      self.source_path.as_ref().or(self.pdf_source_path.as_ref()).cloned()
    else {
      return;
    };
    sync.enqueue_book(BookUpload {
      book_id: book_id.clone(),
      title: crate::library::title_from_path(&path),
      format: crate::library::kind_from_path(&path),
      path,
      // Metadata-only sync registers the record but keeps the file local.
      upload_blob: self.sync_mode.syncs_blob(),
    });
  }

  pub(crate) fn queue_reconcile_sync_state(
    &mut self,
    auto_apply_server_progress: bool,
  ) {
    self.enqueue_current_book_sync();
    self.startup_progress_reconcile = auto_apply_server_progress;
    let Some(updated_at) =
      self.last_local_progress_updated_at.filter(|ts| *ts > 0)
    else {
      return;
    };
    if self.total_lines == 0 {
      return;
    }
    // While a PDF is still opening the buffer is just a one-line splash;
    // pushing `offset + cursor_y` here would upload a bogus position. The
    // real position is reconciled once the stream installs (and flushed on
    // exit), matching the guard in `save_progress_snapshot`.
    if self.pdf_pending.is_some() && self.pdf_streaming.is_none() {
      return;
    }
    let (page, line_in_page) = match self.current_pdf_position() {
      Some((p, l)) => (Some(p), Some(l)),
      None => (None, None),
    };
    self.enqueue_progress_sync_at(
      self.offset + self.cursor_y,
      page,
      line_in_page,
      updated_at,
    );
  }

  /// Queue the current reading position for upload. Cheap: builds a payload and
  /// does one non-blocking channel send. No-op without a server or `book_id`.
  pub(crate) fn enqueue_progress_sync(
    &mut self,
    current_line: usize,
    page: Option<u32>,
    line_in_page: Option<usize>,
  ) {
    self.enqueue_progress_sync_at(
      current_line,
      page,
      line_in_page,
      self.sync_now_ms(),
    );
  }

  pub(crate) fn enqueue_progress_sync_at(
    &mut self,
    current_line: usize,
    page: Option<u32>,
    line_in_page: Option<usize>,
    updated_at: i64,
  ) {
    let (Some(sync), Some(book_id)) =
      (self.sync.as_ref(), self.book_id.as_ref())
    else {
      return;
    };
    // Reading state (progress + annotations) syncs in `full` and `metadata`;
    // `off` keeps everything local.
    if !self.sync_mode.syncs_state() || !self.doc_auto_syncs() {
      return;
    }
    // Exact resume anchor origin: the page's first line for a streaming PDF
    // (page-local, so the peer resolves it from the target page alone), or the
    // document start (line 0) for reflowable formats (global). A reflowable
    // save has no `line_in_page`, so it must anchor from 0 — subtracting
    // nothing would make the range empty and the anchor a useless 0.
    let word_start =
      line_in_page.map_or(0, |lip| current_line.saturating_sub(lip));
    let word_offset = Some(crate::word_anchor::words_in_range(
      &self.lines,
      &self.line_kinds,
      word_start,
      current_line,
    ));
    let payload = ProgressPayload {
      book_id: book_id.clone(),
      offset: current_line,
      total_lines: self.total_lines,
      percentage: self.reading_percent(current_line),
      viewport_offset: Some(self.offset),
      cursor_y: Some(self.cursor_y),
      page,
      line_in_page,
      word_offset,
      op_id: Uuid::new_v4().to_string(),
      updated_at,
    };
    self.last_synced_offset = Some(current_line);
    self.last_local_progress_updated_at = Some(updated_at);
    sync.enqueue_progress(payload);
  }

  /// Queue the book's cumulative active reading time for upload.
  pub(crate) fn enqueue_reading_time_sync(&self, seconds: u64) {
    let (Some(sync), Some(book_id)) =
      (self.sync.as_ref(), self.book_id.as_ref())
    else {
      return;
    };
    if !self.sync_mode.syncs_state() || !self.doc_auto_syncs() {
      return;
    }
    sync.enqueue_reading_time(crate::sync::ReadingTimePayload {
      book_id: book_id.clone(),
      seconds,
      op_id: Uuid::new_v4().to_string(),
      updated_at: self.sync_now_ms(),
    });
  }

  /// Queue today's cumulative active reading seconds (device-wide) for upload.
  pub(crate) fn enqueue_reading_day_sync(&self, day: String, seconds: u64) {
    let (Some(sync), Some(book_id)) =
      (self.sync.as_ref(), self.book_id.as_ref())
    else {
      return;
    };
    if !self.sync_mode.syncs_state() || !self.doc_auto_syncs() {
      return;
    }
    sync.enqueue_reading_day(crate::sync::ReadingDayPayload {
      book_id: book_id.clone(),
      day,
      seconds,
      op_id: Uuid::new_v4().to_string(),
      updated_at: self.sync_now_ms(),
    });
  }

  /// Queue a bookmark add/delete for upload. No-op without a server or book id.
  pub(crate) fn enqueue_bookmark_sync(
    &self,
    mark: char,
    line: usize,
    col: usize,
    deleted: bool,
  ) {
    let (Some(sync), Some(book_id)) =
      (self.sync.as_ref(), self.book_id.as_ref())
    else {
      return;
    };
    if !self.sync_mode.syncs_state() || !self.doc_auto_syncs() {
      return;
    }
    sync.enqueue_annotation(AnnotationOp::bookmark(
      book_id,
      mark,
      line,
      col,
      deleted,
      self.sync_now_ms(),
    ));
  }

  /// Queue a highlight add/delete for upload.
  pub(crate) fn enqueue_highlight_sync(
    &self,
    start: usize,
    end: usize,
    deleted: bool,
  ) {
    let (Some(sync), Some(book_id)) =
      (self.sync.as_ref(), self.book_id.as_ref())
    else {
      return;
    };
    if !self.sync_mode.syncs_state() || !self.doc_auto_syncs() {
      return;
    }
    sync.enqueue_annotation(AnnotationOp::highlight(
      book_id,
      start,
      end,
      deleted,
      self.sync_now_ms(),
    ));
  }

  /// Queue a note add/edit/delete for upload, keyed by the note's stable id.
  pub(crate) fn enqueue_note_sync(
    &self,
    note: &crate::notes::Note,
    deleted: bool,
  ) {
    let (Some(sync), Some(book_id)) =
      (self.sync.as_ref(), self.book_id.as_ref())
    else {
      return;
    };
    if !self.sync_mode.syncs_state() || !self.doc_auto_syncs() {
      return;
    }
    sync.enqueue_annotation(AnnotationOp::note(
      book_id,
      &note.id,
      &note.body,
      note.line,
      note.created_at,
      deleted,
      self.sync_now_ms(),
    ));
  }
}
