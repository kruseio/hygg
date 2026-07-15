//! Editor-facing rows converted from the shared pull DTOs: a synced reading
//! position and a remote-library entry. Both use `usize`/`u32` so the rest of
//! the reader never touches the wire's `i64`.

use hygg_shared::sync::proto;

/// Progress as returned by `GET /api/v1/sync/pull`. Carries just what the
/// editor needs to jump to the position.
#[derive(Clone, Debug)]
pub struct ServerProgress {
  pub book_id: String,
  pub offset: usize,
  /// Line count of the document where this position was recorded. When it
  /// differs from the local document's line count, the sender paginated the
  /// document differently (e.g. the PWA renders PDFs with a different line
  /// count), so the flat `offset` is not directly comparable and the position
  /// is re-mapped by `percentage` instead.
  pub total_lines: usize,
  /// Fraction read (0–100), the pagination-independent coordinate used to
  /// re-map a position synced from a differently-paginated reader.
  pub percentage: f64,
  pub viewport_offset: Option<usize>,
  pub cursor_y: Option<usize>,
  /// 1-based PDF page the saved position lands on (streaming PDFs only). When
  /// present this is the *stable* coordinate to restore by: a flat
  /// `offset`/`viewport_offset` only matches once every page is loaded, but a
  /// (page, line_in_page) pair lands correctly even while pages are still
  /// streaming in. None for non-PDF documents and older server rows.
  pub page: Option<u32>,
  /// Cursor row within the page's rendered output (0-based), paired with
  /// `page`.
  pub line_in_page: Option<usize>,
  /// Non-whitespace character offset of the sender's center line (page-local
  /// when `page` is set, global otherwise) — the exact cross-width anchor.
  pub word_offset: Option<usize>,
  pub updated_at: i64,
}

impl From<proto::ProgressDto> for ServerProgress {
  fn from(d: proto::ProgressDto) -> Self {
    let to_usize = |n: i64| n.max(0) as usize;
    ServerProgress {
      book_id: d.book_id,
      offset: to_usize(d.offset_line),
      total_lines: to_usize(d.total_lines),
      percentage: d.percentage,
      viewport_offset: d.viewport_offset.map(to_usize),
      cursor_y: d.cursor_y.map(to_usize),
      page: d.page.map(|n| n.max(0) as u32),
      line_in_page: d.line_in_page.map(to_usize),
      word_offset: d.word_offset.map(to_usize),
      updated_at: d.updated_at,
    }
  }
}

/// A book in the server library, as returned by `GET /api/v1/books`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteBook {
  pub content_hash: String,
  pub title: String,
  pub author: String,
  pub format: String,
  pub size_bytes: i64,
  pub updated_at: i64,
  /// The account-wide sync ceiling for this document (`full` | `metadata` |
  /// `off`). This device clamps its own preference no higher than this.
  pub sync_mode: proto::SyncMode,
}

impl From<proto::BookDto> for RemoteBook {
  fn from(d: proto::BookDto) -> Self {
    RemoteBook {
      content_hash: d.content_hash,
      title: d.title,
      author: d.author,
      format: d.format,
      size_bytes: d.size_bytes,
      updated_at: d.updated_at,
      sync_mode: d.sync_mode,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn remote_book_converts_from_dto() {
    let book = RemoteBook::from(proto::BookDto {
      content_hash: "abc".into(),
      title: "Dune".into(),
      author: "FH".into(),
      format: "epub".into(),
      size_bytes: 1234,
      updated_at: 99,
      sync_mode: proto::SyncMode::Metadata,
    });
    assert_eq!(book.content_hash, "abc");
    assert_eq!(book.title, "Dune");
    assert_eq!(book.format, "epub");
    assert_eq!(book.size_bytes, 1234);
    assert_eq!(book.sync_mode, proto::SyncMode::Metadata);
  }

  #[test]
  fn server_progress_converts_from_dto() {
    let sp = ServerProgress::from(proto::ProgressDto {
      book_id: "b".into(),
      offset_line: 120,
      total_lines: 900,
      percentage: 13.3,
      viewport_offset: Some(100),
      cursor_y: Some(5),
      page: Some(8),
      line_in_page: Some(3),
      word_offset: Some(42),
      updated_at: 1000,
    });
    assert_eq!(sp.book_id, "b");
    assert_eq!(sp.offset, 120);
    assert_eq!(sp.total_lines, 900);
    assert_eq!(sp.percentage, 13.3);
    assert_eq!(sp.viewport_offset, Some(100));
    assert_eq!(sp.page, Some(8));
    assert_eq!(sp.line_in_page, Some(3));
    assert_eq!(sp.word_offset, Some(42));
    assert_eq!(sp.updated_at, 1000);
  }

  #[test]
  fn server_progress_page_fields_default_to_none() {
    let sp = ServerProgress::from(proto::ProgressDto {
      book_id: "b".into(),
      offset_line: 5,
      total_lines: 0,
      percentage: 0.0,
      viewport_offset: None,
      cursor_y: None,
      page: None,
      line_in_page: None,
      word_offset: None,
      updated_at: 1,
    });
    assert_eq!(sp.page, None);
    assert_eq!(sp.line_in_page, None);
    assert_eq!(sp.word_offset, None);
  }
}
