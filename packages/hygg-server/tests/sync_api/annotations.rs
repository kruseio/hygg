use serde_json::json;

use crate::helpers::*;

#[tokio::test]
async fn bookmark_syncs_and_delete_tombstones_propagate() {
  let (_dir, state) = setup().await;
  let device_a = register_device(&state).await;
  let device_b = register_device(&state).await;

  let add = json!({
    "op_id": "bm1", "kind": "bookmark", "book_id": "book-1",
    "updated_at": 1000, "data": { "mark": "a", "line": 42, "col": 3 }
  });
  push(&state, &device_a, json!({ "ops": [add] })).await;

  let pulled = pull(&state, &device_b, 0).await;
  let bookmarks = pulled["bookmarks"].as_array().unwrap();
  assert_eq!(bookmarks.len(), 1);
  assert_eq!(bookmarks[0]["mark"], "a");
  assert_eq!(bookmarks[0]["line"], 42);
  assert_eq!(bookmarks[0]["deleted"], false);

  // Deleting the same mark is a tombstone the other device can apply.
  let del = json!({
    "op_id": "bm2", "kind": "bookmark", "book_id": "book-1",
    "deleted": true, "updated_at": 2000, "data": { "mark": "a" }
  });
  push(&state, &device_a, json!({ "ops": [del] })).await;

  let pulled = pull(&state, &device_b, 0).await;
  let bookmarks = pulled["bookmarks"].as_array().unwrap();
  assert_eq!(bookmarks.len(), 1);
  assert_eq!(bookmarks[0]["deleted"], true);
}

#[tokio::test]
async fn highlight_round_trips_across_devices() {
  let (_dir, state) = setup().await;
  let device_a = register_device(&state).await;
  let device_b = register_device(&state).await;

  let op = json!({
    "op_id": "hl1", "kind": "highlight", "book_id": "book-1",
    "updated_at": 1000,
    "data": { "start_offset": 100, "end_offset": 220 }
  });
  push(&state, &device_a, json!({ "ops": [op] })).await;

  let pulled = pull(&state, &device_b, 0).await;
  let highlights = pulled["highlights"].as_array().unwrap();
  assert_eq!(highlights.len(), 1);
  assert_eq!(highlights[0]["start_offset"], 100);
  assert_eq!(highlights[0]["end_offset"], 220);
}

#[tokio::test]
async fn note_round_trips_and_edit_wins_by_updated_at() {
  let (_dir, state) = setup().await;
  let device_a = register_device(&state).await;
  let device_b = register_device(&state).await;

  let create = json!({
    "op_id": "n1", "kind": "note", "book_id": "book-1", "updated_at": 1000,
    "data": { "id": "note-uuid-1", "body": "first", "line": 7,
              "created_at": 1000 }
  });
  push(&state, &device_a, json!({ "ops": [create] })).await;

  // An edit (same note id, newer updated_at) replaces the body in place.
  let edit = json!({
    "op_id": "n2", "kind": "note", "book_id": "book-1", "updated_at": 2000,
    "data": { "id": "note-uuid-1", "body": "edited", "line": 7,
              "created_at": 1000 }
  });
  push(&state, &device_b, json!({ "ops": [edit] })).await;

  let pulled = pull(&state, &device_a, 0).await;
  let notes = pulled["notes"].as_array().unwrap();
  assert_eq!(notes.len(), 1);
  assert_eq!(notes[0]["id"], "note-uuid-1");
  assert_eq!(notes[0]["body"], "edited");
  assert_eq!(notes[0]["anchor_line"], 7);
}
