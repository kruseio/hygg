//! Outbound sync ops: the push envelope and its typed, `kind`-tagged payloads.

use serde::{Deserialize, Serialize};

/// `POST /api/v1/sync/push` request body: a batch of self-describing ops.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushRequest {
  #[serde(default)]
  pub device_id: Option<String>,
  pub ops: Vec<SyncOp>,
}

/// `POST /api/v1/sync/push` response body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushResponse {
  /// Ops newly applied this request.
  pub applied: Vec<String>,
  /// Ops skipped (already applied, or not permitted / understood).
  pub skipped: Vec<String>,
  pub server_time: i64,
}

/// A single sync operation. The envelope fields are common to every kind; the
/// `kind`-tagged [`OpPayload`] carries the per-kind data. Conflict policy is
/// last-write-wins by `updated_at`, with `op_id` providing idempotency.
///
/// On the wire the payload flattens into the envelope, so an op is
/// `{"op_id":…,"book_id":…,"deleted":…,"updated_at":…,"kind":…,"data":{…}}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncOp {
  pub op_id: String,
  pub book_id: String,
  #[serde(default)]
  pub deleted: bool,
  pub updated_at: i64,
  #[serde(flatten)]
  pub payload: OpPayload,
}

/// The typed payload of a [`SyncOp`], serialised as `"kind"` + `"data"` sibling
/// fields (e.g. `{"kind":"progress","data":{…}}`). A new entity kind is a new
/// variant here, and an unhandled variant is a compile error on the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum OpPayload {
  Progress(ProgressData),
  Bookmark(BookmarkData),
  Highlight(HighlightData),
  Note(NoteData),
  ReadingTime(ReadingTimeData),
  ReadingDay(ReadingDayData),
}

/// `data` for a `progress` op: the latest read position for a document.
///
/// Scalar fields carry `#[serde(default)]` so a partial or older client (or one
/// that only knows `offset`) still applies — matching the server's
/// long-standing "missing scalar defaults to 0" handling. The real client sends
/// them all.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressData {
  #[serde(default)]
  pub offset: u64,
  #[serde(default)]
  pub total_lines: u64,
  #[serde(default)]
  pub percentage: f64,
  pub viewport_offset: Option<u64>,
  pub cursor_y: Option<u64>,
  /// 1-based PDF page the position lands on (streaming PDFs only).
  pub page: Option<u32>,
  pub line_in_page: Option<u64>,
  /// The rendering-independent resume anchor: the count of non-whitespace
  /// characters before the viewport-center line (image rows excluded),
  /// page-local when `page` is set and global otherwise. The same content
  /// yields the same count at any wrap width and with image rendering on or
  /// off, so a peer resolves it back to the *exact* same line. (The field name
  /// is historical — it once held a whitespace-delimited word index, which the
  /// justifier's width-dependent hard-splitting made non-portable.) See
  /// [`hygg_shared::anchor`].
  #[serde(default)]
  pub word_offset: Option<u64>,
}

/// `data` for a `bookmark` op (the `deleted` flag rides on the envelope). A
/// tombstone is keyed by `mark` alone, so the position defaults to 0 when
/// absent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmarkData {
  pub mark: String,
  #[serde(default)]
  pub line: u64,
  #[serde(default)]
  pub col: u64,
}

/// `data` for a `highlight` op. The span identifies the highlight.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HighlightData {
  #[serde(default)]
  pub start_offset: u64,
  #[serde(default)]
  pub end_offset: u64,
  /// Absent on add; the server falls back to the op's `updated_at`.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub created_at: Option<i64>,
}

/// `data` for a `note` op. `id` is the client's stable note uuid.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteData {
  pub id: String,
  #[serde(default)]
  pub body: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub line: Option<u64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub created_at: Option<i64>,
}

/// `data` for a `reading_time` op: cumulative active seconds for a book.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadingTimeData {
  #[serde(default)]
  pub seconds: u64,
}

/// `data` for a `reading_day` op: cumulative active seconds for a calendar day.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadingDayData {
  pub day: String,
  #[serde(default)]
  pub seconds: u64,
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;

  #[test]
  fn progress_op_round_trips_to_expected_wire_shape() {
    let op = SyncOp {
      op_id: "op".into(),
      book_id: "b".into(),
      deleted: false,
      updated_at: 42,
      payload: OpPayload::Progress(ProgressData {
        offset: 10,
        total_lines: 100,
        percentage: 10.0,
        viewport_offset: Some(5),
        cursor_y: Some(2),
        page: None,
        line_in_page: None,
        word_offset: None,
      }),
    };
    let value = serde_json::to_value(&op).unwrap();
    assert_eq!(value["op_id"], "op");
    assert_eq!(value["kind"], "progress");
    assert_eq!(value["book_id"], "b");
    assert_eq!(value["updated_at"], 42);
    assert_eq!(value["data"]["offset"], 10);
    assert_eq!(value["data"]["total_lines"], 100);

    let back: SyncOp = serde_json::from_value(value).unwrap();
    assert_eq!(back.op_id, "op");
    assert!(matches!(back.payload, OpPayload::Progress(_)));
  }

  #[test]
  fn note_tombstone_uses_snake_case_kind_and_envelope_deleted() {
    let op = SyncOp {
      op_id: "op".into(),
      book_id: "b".into(),
      deleted: true,
      updated_at: 200,
      payload: OpPayload::Note(NoteData {
        id: "id1".into(),
        body: String::new(),
        line: None,
        created_at: Some(1),
      }),
    };
    let value = serde_json::to_value(&op).unwrap();
    assert_eq!(value["kind"], "note");
    assert_eq!(value["deleted"], true);
    assert_eq!(value["data"]["id"], "id1");
    assert!(value["data"].get("line").is_none());
  }

  #[test]
  fn reading_time_kind_is_snake_case() {
    let op = SyncOp {
      op_id: "op".into(),
      book_id: "b".into(),
      deleted: false,
      updated_at: 42,
      payload: OpPayload::ReadingTime(ReadingTimeData { seconds: 1234 }),
    };
    let value = serde_json::to_value(&op).unwrap();
    assert_eq!(value["kind"], "reading_time");
    assert_eq!(value["data"]["seconds"], 1234);
  }

  #[test]
  fn unknown_kind_is_a_clean_deserialize_error() {
    let value = json!({
      "op_id": "o", "book_id": "b", "updated_at": 1,
      "kind": "telepathy", "data": {}
    });
    assert!(serde_json::from_value::<SyncOp>(value).is_err());
  }
}
