//! Messages and payloads exchanged between the editor (main thread) and the
//! background sync engine. Outbound payloads serialise to the shared wire ops
//! (`hygg_shared::sync::proto`); inbound rows are converted from the shared
//! pull DTOs into these editor-facing shapes, which use `usize`/`u32` and keep
//! the rest of the reader free of wire concerns.

use hygg_shared::sync::proto;

use super::annotations::{ServerBookmark, ServerHighlight, ServerNote};
use super::inbound::ServerProgress;

/// A local document that should be present on the server before progress or
/// annotations reference it.
#[derive(Clone, Debug)]
pub struct BookUpload {
  pub book_id: String,
  pub title: String,
  pub format: String,
  pub path: String,
  /// Whether to upload the document bytes. `true` in full sync; `false` in
  /// metadata-only sync, where the record is registered but the file stays on
  /// this device.
  pub upload_blob: bool,
}

/// A progress update queued for upload.
#[derive(Clone, Debug)]
pub struct ProgressPayload {
  pub book_id: String,
  pub offset: usize,
  pub total_lines: usize,
  pub percentage: f64,
  pub viewport_offset: Option<usize>,
  pub cursor_y: Option<usize>,
  pub page: Option<u32>,
  pub line_in_page: Option<usize>,
  /// Non-whitespace character offset of the viewport-center line (page-local
  /// for PDFs, global otherwise) — the exact cross-width resume anchor.
  pub word_offset: Option<usize>,
  pub op_id: String,
  pub updated_at: i64,
}

impl ProgressPayload {
  /// Serialise to a typed push op for `POST /api/v1/sync/push`.
  pub fn to_op(&self) -> proto::SyncOp {
    proto::SyncOp {
      op_id: self.op_id.clone(),
      book_id: self.book_id.clone(),
      deleted: false,
      updated_at: self.updated_at,
      payload: proto::OpPayload::Progress(proto::ProgressData {
        offset: self.offset as u64,
        total_lines: self.total_lines as u64,
        percentage: self.percentage,
        viewport_offset: self.viewport_offset.map(|n| n as u64),
        cursor_y: self.cursor_y.map(|n| n as u64),
        page: self.page,
        line_in_page: self.line_in_page.map(|n| n as u64),
        word_offset: self.word_offset.map(|n| n as u64),
      }),
    }
  }
}

/// Cumulative active reading time for a book on this device, queued for upload.
/// The server keeps one row per (book, device) and sums across devices.
#[derive(Clone, Debug)]
pub struct ReadingTimePayload {
  pub book_id: String,
  pub seconds: u64,
  pub op_id: String,
  pub updated_at: i64,
}

impl ReadingTimePayload {
  pub fn to_op(&self) -> proto::SyncOp {
    proto::SyncOp {
      op_id: self.op_id.clone(),
      book_id: self.book_id.clone(),
      deleted: false,
      updated_at: self.updated_at,
      payload: proto::OpPayload::ReadingTime(proto::ReadingTimeData {
        seconds: self.seconds,
      }),
    }
  }
}

/// Cumulative active reading seconds for a single calendar day on this device,
/// queued for upload. Drives the server-side reading streak. `book_id` carries
/// the book in view (so the server's per-book write gate passes); aggregation
/// is per (device, day).
#[derive(Clone, Debug)]
pub struct ReadingDayPayload {
  pub book_id: String,
  pub day: String,
  pub seconds: u64,
  pub op_id: String,
  pub updated_at: i64,
}

impl ReadingDayPayload {
  pub fn to_op(&self) -> proto::SyncOp {
    proto::SyncOp {
      op_id: self.op_id.clone(),
      book_id: self.book_id.clone(),
      deleted: false,
      updated_at: self.updated_at,
      payload: proto::OpPayload::ReadingDay(proto::ReadingDayData {
        day: self.day.clone(),
        seconds: self.seconds,
      }),
    }
  }
}

/// Editor -> engine commands. `SseUp`/`SseDown` are sent by the SSE listener
/// thread to switch the engine between push-driven (slow safety-net polling)
/// and poll-only (fast polling) modes.
pub enum SyncCmd {
  EnqueueBook(BookUpload),
  EnqueueProgress(ProgressPayload),
  EnqueueAnnotation(proto::SyncOp),
  EnqueueReadingTime(ReadingTimePayload),
  EnqueueReadingDay(ReadingDayPayload),
  FlushNow {
    report: bool,
  },
  PullNow,
  /// A one-off *full* pull (from cursor 0), used by `:server-progress` so the
  /// current server position is re-delivered even when it is unchanged since
  /// our delta cursor (a plain `PullNow` would return nothing). Does not
  /// disturb the engine's real cursor.
  RefetchProgress,
  SseUp,
  SseDown,
  Shutdown,
}

/// Engine -> editor notifications, drained once per render tick. Each carries a
/// single changed row for the editor to act on (jump prompt for progress;
/// apply-and-persist for annotations of the current book).
pub enum SyncEvent {
  Status { ok: bool, message: String },
  SyncCycleComplete,
  Progress(ServerProgress),
  Bookmark(ServerBookmark),
  Highlight(ServerHighlight),
  Note(ServerNote),
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn progress_payload_serializes_expected_op_shape() {
    let payload = ProgressPayload {
      book_id: "b".into(),
      offset: 10,
      total_lines: 100,
      percentage: 10.0,
      viewport_offset: Some(5),
      cursor_y: Some(2),
      page: None,
      line_in_page: None,
      word_offset: Some(7),
      op_id: "op".into(),
      updated_at: 42,
    };
    let op = serde_json::to_value(payload.to_op()).unwrap();
    assert_eq!(op["kind"], "progress");
    assert_eq!(op["book_id"], "b");
    assert_eq!(op["data"]["offset"], 10);
    assert_eq!(op["updated_at"], 42);
  }

  #[test]
  fn reading_time_op_has_expected_shape() {
    let op = serde_json::to_value(
      ReadingTimePayload {
        book_id: "b".into(),
        seconds: 1234,
        op_id: "op".into(),
        updated_at: 42,
      }
      .to_op(),
    )
    .unwrap();
    assert_eq!(op["kind"], "reading_time");
    assert_eq!(op["book_id"], "b");
    assert_eq!(op["data"]["seconds"], 1234);
    assert_eq!(op["updated_at"], 42);
  }

  #[test]
  fn reading_day_op_has_expected_shape() {
    let op = serde_json::to_value(
      ReadingDayPayload {
        book_id: "b".into(),
        day: "2026-06-25".into(),
        seconds: 600,
        op_id: "op".into(),
        updated_at: 42,
      }
      .to_op(),
    )
    .unwrap();
    assert_eq!(op["kind"], "reading_day");
    assert_eq!(op["data"]["day"], "2026-06-25");
    assert_eq!(op["data"]["seconds"], 600);
  }
}
