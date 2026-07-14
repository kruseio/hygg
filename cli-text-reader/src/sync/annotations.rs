//! Bookmark/highlight/note sync. Outbound changes are built as shared `proto`
//! push ops via [`AnnotationOp`]'s constructors; inbound rows from a pull are
//! the editor-facing `Server*` types, converted from the shared pull DTOs.
//! Unlike progress (coalesced to the latest per book), each annotation add or
//! delete is its own idempotent op so deletions propagate as tombstones.

use hygg_shared::sync::proto;
use uuid::Uuid;

/// Constructors for annotation push ops. Each returns a shared
/// [`proto::SyncOp`]; the deletion flag rides on the op envelope.
pub struct AnnotationOp;

impl AnnotationOp {
  pub fn bookmark(
    book_id: &str,
    mark: char,
    line: usize,
    col: usize,
    deleted: bool,
    updated_at: i64,
  ) -> proto::SyncOp {
    proto::SyncOp {
      op_id: new_op_id(),
      book_id: book_id.to_string(),
      deleted,
      updated_at,
      payload: proto::OpPayload::Bookmark(proto::BookmarkData {
        mark: mark.to_string(),
        line: line as u64,
        col: col as u64,
      }),
    }
  }

  pub fn highlight(
    book_id: &str,
    start: usize,
    end: usize,
    deleted: bool,
    updated_at: i64,
  ) -> proto::SyncOp {
    proto::SyncOp {
      op_id: new_op_id(),
      book_id: book_id.to_string(),
      deleted,
      updated_at,
      payload: proto::OpPayload::Highlight(proto::HighlightData {
        start_offset: start as u64,
        end_offset: end as u64,
        created_at: None,
      }),
    }
  }

  pub fn note(
    book_id: &str,
    id: &str,
    body: &str,
    line: Option<usize>,
    created_at: i64,
    deleted: bool,
    updated_at: i64,
  ) -> proto::SyncOp {
    proto::SyncOp {
      op_id: new_op_id(),
      book_id: book_id.to_string(),
      deleted,
      updated_at,
      payload: proto::OpPayload::Note(proto::NoteData {
        id: id.to_string(),
        body: body.to_string(),
        line: line.map(|n| n as u64),
        created_at: Some(created_at),
      }),
    }
  }
}

fn new_op_id() -> String {
  Uuid::new_v4().to_string()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerBookmark {
  pub book_id: String,
  pub mark: String,
  pub line: usize,
  pub col: usize,
  pub deleted: bool,
}

impl From<proto::BookmarkDto> for ServerBookmark {
  fn from(d: proto::BookmarkDto) -> Self {
    ServerBookmark {
      book_id: d.book_id,
      mark: d.mark,
      line: d.line.max(0) as usize,
      col: d.col.max(0) as usize,
      deleted: d.deleted,
    }
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerHighlight {
  pub book_id: String,
  pub start: usize,
  pub end: usize,
  pub deleted: bool,
}

impl From<proto::HighlightDto> for ServerHighlight {
  fn from(d: proto::HighlightDto) -> Self {
    ServerHighlight {
      book_id: d.book_id,
      start: d.start_offset.max(0) as usize,
      end: d.end_offset.max(0) as usize,
      deleted: d.deleted,
    }
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerNote {
  pub book_id: String,
  pub id: String,
  pub body: String,
  pub line: Option<usize>,
  pub created_at: i64,
  pub updated_at: i64,
  pub deleted: bool,
}

impl From<proto::NoteDto> for ServerNote {
  fn from(d: proto::NoteDto) -> Self {
    ServerNote {
      book_id: d.book_id,
      id: d.id,
      body: d.body,
      line: d.anchor_line.map(|n| n.max(0) as usize),
      created_at: d.created_at,
      updated_at: d.updated_at,
      deleted: d.deleted,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn bookmark_op_has_expected_shape() {
    let op =
      serde_json::to_value(AnnotationOp::bookmark("b", 'a', 42, 3, false, 100))
        .unwrap();
    assert_eq!(op["kind"], "bookmark");
    assert_eq!(op["book_id"], "b");
    assert_eq!(op["deleted"], false);
    assert_eq!(op["data"]["mark"], "a");
    assert_eq!(op["data"]["line"], 42);
  }

  #[test]
  fn note_delete_op_is_a_tombstone() {
    let op = serde_json::to_value(AnnotationOp::note(
      "b", "id1", "", None, 1, true, 200,
    ))
    .unwrap();
    assert_eq!(op["kind"], "note");
    assert_eq!(op["deleted"], true);
    assert_eq!(op["data"]["id"], "id1");
  }

  #[test]
  fn server_bookmark_converts_and_reads_deleted_flag() {
    let bm = ServerBookmark::from(proto::BookmarkDto {
      book_id: "b".into(),
      mark: "a".into(),
      line: 7,
      col: 2,
      deleted: true,
      updated_at: 1,
    });
    assert_eq!(bm.mark, "a");
    assert_eq!(bm.line, 7);
    assert!(bm.deleted);
  }

  #[test]
  fn server_highlight_converts_offsets() {
    let hl = ServerHighlight::from(proto::HighlightDto {
      book_id: "b".into(),
      start_offset: 10,
      end_offset: 20,
      deleted: false,
      updated_at: 1,
    });
    assert_eq!(hl.start, 10);
    assert_eq!(hl.end, 20);
    assert!(!hl.deleted);
  }

  #[test]
  fn server_note_maps_anchor_line_to_option() {
    let note = ServerNote::from(proto::NoteDto {
      id: "n1".into(),
      book_id: "b".into(),
      anchor_line: Some(5),
      body: "hi".into(),
      deleted: false,
      created_at: 1,
      updated_at: 2,
    });
    assert_eq!(note.id, "n1");
    assert_eq!(note.line, Some(5));
    assert_eq!(note.body, "hi");
  }
}
