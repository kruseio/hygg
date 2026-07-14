//! Mappings from the server's internal rows and principal into the shared wire
//! DTOs (`hygg_shared::sync::proto`). The API speaks only those DTOs, so every
//! response is built by converting a local type here — keeping the JSON
//! contract in one shared place and out of the handlers.

use hygg_shared::sync::proto;

use crate::auth::Principal;
use crate::repo::bookmarks::BookmarkRow;
use crate::repo::books::BookRow;
use crate::repo::devices::DeviceSummary;
use crate::repo::highlights::HighlightRow;
use crate::repo::notes::NoteRow;
use crate::repo::progress::ProgressRow;

impl From<&Principal> for proto::MeResponse {
  fn from(p: &Principal) -> Self {
    proto::MeResponse {
      tenant_id: p.tenant_id.clone(),
      user_id: p.user_id.clone(),
      device_id: p.device_id.clone(),
      is_admin: p.role.is_admin(),
      default_access: p.default_access.as_str().to_string(),
      read_only: p.read_only,
      progress_sync_denied: p.progress_sync_denied,
      // Filled in by the `me` handler, which has the DB access an extension
      // needs to answer.
      label: None,
    }
  }
}

impl From<DeviceSummary> for proto::DeviceDto {
  fn from(d: DeviceSummary) -> Self {
    proto::DeviceDto {
      id: d.id,
      name: d.name,
      platform: d.platform,
      default_access: d.default_access,
      read_only: d.read_only != 0,
      progress_sync_denied: d.progress_sync_denied != 0,
      revoked: d.revoked != 0,
      created_at: d.created_at,
      last_seen_at: d.last_seen_at,
    }
  }
}

impl From<BookRow> for proto::BookDto {
  fn from(b: BookRow) -> Self {
    proto::BookDto {
      content_hash: b.content_hash,
      title: b.title,
      author: b.author,
      format: b.format,
      size_bytes: b.size_bytes,
      updated_at: b.updated_at,
      sync_mode: proto::SyncMode::from_token_or_default(&b.sync_mode),
    }
  }
}

impl From<ProgressRow> for proto::ProgressDto {
  fn from(r: ProgressRow) -> Self {
    proto::ProgressDto {
      book_id: r.book_id,
      offset_line: r.offset_line,
      total_lines: r.total_lines,
      percentage: r.percentage,
      viewport_offset: r.viewport_offset,
      cursor_y: r.cursor_y,
      page: r.page,
      line_in_page: r.line_in_page,
      word_offset: r.word_offset,
      updated_at: r.updated_at,
    }
  }
}

impl From<BookmarkRow> for proto::BookmarkDto {
  fn from(r: BookmarkRow) -> Self {
    proto::BookmarkDto {
      book_id: r.book_id,
      mark: r.mark,
      line: r.line,
      col: r.col,
      deleted: r.deleted != 0,
      updated_at: r.updated_at,
    }
  }
}

impl From<HighlightRow> for proto::HighlightDto {
  fn from(r: HighlightRow) -> Self {
    proto::HighlightDto {
      book_id: r.book_id,
      start_offset: r.start_offset,
      end_offset: r.end_offset,
      deleted: r.deleted != 0,
      updated_at: r.updated_at,
    }
  }
}

impl From<NoteRow> for proto::NoteDto {
  fn from(r: NoteRow) -> Self {
    proto::NoteDto {
      id: r.id,
      book_id: r.book_id,
      anchor_line: r.anchor_line,
      body: r.body,
      deleted: r.deleted != 0,
      created_at: r.created_at,
      updated_at: r.updated_at,
    }
  }
}
