//! Translations from wire `SyncOp` payloads to the repo input structs, kept
//! beside `sync.rs` to keep that handler module small.

use hygg_shared::sync::proto::{self, SyncOp};

use crate::auth::Principal;
use crate::repo::bookmarks::BookmarkInput;
use crate::repo::highlights::HighlightInput;
use crate::repo::notes::NoteInput;
use crate::repo::progress::ProgressInput;
use crate::repo::reading::{ReadingDayInput, ReadingTimeInput};

pub(crate) fn progress_input(
  principal: &Principal,
  op: &SyncOp,
  data: &proto::ProgressData,
) -> ProgressInput {
  ProgressInput {
    book_id: op.book_id.clone(),
    device_id: Some(principal.device_id.clone()),
    offset_line: data.offset as i64,
    total_lines: data.total_lines as i64,
    percentage: data.percentage,
    viewport_offset: data.viewport_offset.map(|n| n as i64),
    cursor_y: data.cursor_y.map(|n| n as i64),
    page: data.page.map(|n| n as i64),
    line_in_page: data.line_in_page.map(|n| n as i64),
    word_offset: data.word_offset.map(|n| n as i64),
    op_id: op.op_id.clone(),
    updated_at: op.updated_at,
  }
}

pub(crate) fn bookmark_input(
  principal: &Principal,
  op: &SyncOp,
  data: &proto::BookmarkData,
) -> BookmarkInput {
  BookmarkInput {
    book_id: op.book_id.clone(),
    device_id: Some(principal.device_id.clone()),
    mark: data.mark.clone(),
    line: data.line as i64,
    col: data.col as i64,
    op_id: op.op_id.clone(),
    deleted: op.deleted,
    updated_at: op.updated_at,
  }
}

pub(crate) fn highlight_input(
  principal: &Principal,
  op: &SyncOp,
  data: &proto::HighlightData,
) -> HighlightInput {
  HighlightInput {
    book_id: op.book_id.clone(),
    device_id: Some(principal.device_id.clone()),
    start_offset: data.start_offset as i64,
    end_offset: data.end_offset as i64,
    op_id: op.op_id.clone(),
    deleted: op.deleted,
    created_at: data.created_at.unwrap_or(op.updated_at),
    updated_at: op.updated_at,
  }
}

pub(crate) fn note_input(
  principal: &Principal,
  op: &SyncOp,
  data: &proto::NoteData,
) -> NoteInput {
  NoteInput {
    note_uid: data.id.clone(),
    book_id: op.book_id.clone(),
    device_id: Some(principal.device_id.clone()),
    anchor_line: data.line.map(|n| n as i64),
    body: data.body.clone(),
    op_id: op.op_id.clone(),
    deleted: op.deleted,
    created_at: data.created_at.unwrap_or(op.updated_at),
    updated_at: op.updated_at,
  }
}

pub(crate) fn reading_time_input(
  principal: &Principal,
  op: &SyncOp,
  data: &proto::ReadingTimeData,
) -> ReadingTimeInput {
  ReadingTimeInput {
    book_id: op.book_id.clone(),
    device_id: principal.device_id.clone(),
    seconds: data.seconds as i64,
    op_id: op.op_id.clone(),
    updated_at: op.updated_at,
  }
}

pub(crate) fn reading_day_input(
  principal: &Principal,
  op: &SyncOp,
  data: &proto::ReadingDayData,
) -> ReadingDayInput {
  ReadingDayInput {
    device_id: principal.device_id.clone(),
    day: data.day.clone(),
    seconds: data.seconds as i64,
    op_id: op.op_id.clone(),
    updated_at: op.updated_at,
  }
}
