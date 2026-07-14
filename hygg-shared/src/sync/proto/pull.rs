//! Inbound rows: the pull cursor and the per-kind rows a pull returns.

use serde::{Deserialize, Serialize};

/// `GET /api/v1/sync/pull` query string: the client's cursor (epoch millis).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PullQuery {
  #[serde(default)]
  pub since: Option<i64>,
}

/// `GET /api/v1/sync/pull` response body: everything changed since the cursor.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PullResponse {
  pub server_time: i64,
  pub progress: Vec<ProgressDto>,
  pub bookmarks: Vec<BookmarkDto>,
  pub highlights: Vec<HighlightDto>,
  pub notes: Vec<NoteDto>,
}

/// A progress row from a pull. Mirrors the server's stored columns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressDto {
  pub book_id: String,
  pub offset_line: i64,
  pub total_lines: i64,
  pub percentage: f64,
  pub viewport_offset: Option<i64>,
  pub cursor_y: Option<i64>,
  pub page: Option<i64>,
  pub line_in_page: Option<i64>,
  /// Non-whitespace character offset of the viewport-center line (image rows
  /// excluded) — the exact, rendering-independent resume anchor, page-local
  /// when `page` is set. See `push::ProgressData` and [`hygg_shared::anchor`].
  #[serde(default)]
  pub word_offset: Option<i64>,
  pub updated_at: i64,
}

/// A bookmark row from a pull.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmarkDto {
  pub book_id: String,
  pub mark: String,
  pub line: i64,
  pub col: i64,
  pub deleted: bool,
  pub updated_at: i64,
}

/// A highlight row from a pull.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HighlightDto {
  pub book_id: String,
  pub start_offset: i64,
  pub end_offset: i64,
  pub deleted: bool,
  pub updated_at: i64,
}

/// A note row from a pull. `id` is the client's stable note uuid.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteDto {
  pub id: String,
  pub book_id: String,
  pub anchor_line: Option<i64>,
  pub body: String,
  pub deleted: bool,
  pub created_at: i64,
  pub updated_at: i64,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn pull_response_round_trips() {
    let response = PullResponse {
      server_time: 7,
      progress: vec![ProgressDto {
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
      }],
      bookmarks: vec![BookmarkDto {
        book_id: "b".into(),
        mark: "a".into(),
        line: 7,
        col: 2,
        deleted: true,
        updated_at: 1,
      }],
      highlights: Vec::new(),
      notes: Vec::new(),
    };
    let text = serde_json::to_string(&response).unwrap();
    let back: PullResponse = serde_json::from_str(&text).unwrap();
    assert_eq!(back.server_time, 7);
    assert_eq!(back.progress[0].offset_line, 120);
    assert!(back.bookmarks[0].deleted);
  }
}
