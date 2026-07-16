//! DTO → repo-input conversions for the import half of the export/import flow.
//!
//! Split out of `export.rs` to keep that module within the line budget; these
//! are plain field mappings with one shared rule (see the doc comment on
//! [`progress_input`]).

use hygg_shared::sync::proto::{
  BookmarkDto, HighlightDto, NoteDto, ProgressDto,
};

use crate::repo;
use crate::util::new_id;

/// Imported annotations are not tied to a device (the export dropped device
/// identity) and get a fresh idempotency id; `updated_at` is preserved so
/// last-write-wins keeps the exporting server's ordering.
pub(super) fn progress_input(
  dto: ProgressDto,
) -> repo::progress::ProgressInput {
  repo::progress::ProgressInput {
    book_id: dto.book_id,
    device_id: None,
    offset_line: dto.offset_line,
    total_lines: dto.total_lines,
    percentage: dto.percentage,
    viewport_offset: dto.viewport_offset,
    cursor_y: dto.cursor_y,
    page: dto.page,
    line_in_page: dto.line_in_page,
    word_offset: dto.word_offset,
    op_id: new_id(),
    updated_at: dto.updated_at,
  }
}

pub(super) fn bookmark_input(
  dto: BookmarkDto,
) -> repo::bookmarks::BookmarkInput {
  repo::bookmarks::BookmarkInput {
    book_id: dto.book_id,
    device_id: None,
    mark: dto.mark,
    line: dto.line,
    col: dto.col,
    op_id: new_id(),
    deleted: dto.deleted,
    updated_at: dto.updated_at,
  }
}

pub(super) fn highlight_input(
  dto: HighlightDto,
) -> repo::highlights::HighlightInput {
  repo::highlights::HighlightInput {
    book_id: dto.book_id,
    device_id: None,
    start_offset: dto.start_offset,
    end_offset: dto.end_offset,
    op_id: new_id(),
    deleted: dto.deleted,
    created_at: dto.updated_at,
    updated_at: dto.updated_at,
  }
}

pub(super) fn note_input(dto: NoteDto) -> repo::notes::NoteInput {
  repo::notes::NoteInput {
    note_uid: dto.id,
    book_id: dto.book_id,
    device_id: None,
    anchor_line: dto.anchor_line,
    body: dto.body,
    op_id: new_id(),
    deleted: dto.deleted,
    created_at: dto.created_at,
    updated_at: dto.updated_at,
  }
}
