use axum::http::StatusCode;
use serde_json::json;

use crate::helpers::*;

#[tokio::test]
async fn progress_pushed_by_one_device_is_pulled_by_another() {
  let (_dir, state) = setup().await;
  let device_a = register_device(&state).await;
  let device_b = register_device(&state).await;

  let resp =
    push(&state, &device_a, json!({ "ops": [progress_op("op1", 120, 1000)] }))
      .await;
  assert_eq!(resp.status(), StatusCode::OK);
  assert_eq!(json_body(resp).await["applied"], json!(["op1"]));

  let pulled = pull(&state, &device_b, 0).await;
  let progress = pulled["progress"].as_array().unwrap();
  assert_eq!(progress.len(), 1);
  assert_eq!(progress[0]["book_id"], "book-1");
  assert_eq!(progress[0]["offset_line"], 120);
}

#[tokio::test]
async fn resent_op_is_idempotent() {
  let (_dir, state) = setup().await;
  let token = register_device(&state).await;

  let first =
    push(&state, &token, json!({ "ops": [progress_op("dup", 5, 10)] })).await;
  assert_eq!(json_body(first).await["applied"], json!(["dup"]));

  let second =
    push(&state, &token, json!({ "ops": [progress_op("dup", 5, 10)] })).await;
  let body = json_body(second).await;
  assert_eq!(body["applied"], json!([]));
  assert_eq!(body["skipped"], json!(["dup"]));
}

#[tokio::test]
async fn older_update_does_not_overwrite_newer_progress() {
  let (_dir, state) = setup().await;
  let token = register_device(&state).await;

  push(&state, &token, json!({ "ops": [progress_op("newer", 200, 5000)] }))
    .await;
  // An older op for the same book must not move progress backwards.
  push(&state, &token, json!({ "ops": [progress_op("older", 50, 1000)] }))
    .await;

  let pulled = pull(&state, &token, 0).await;
  let progress = pulled["progress"].as_array().unwrap();
  assert_eq!(progress.len(), 1);
  assert_eq!(progress[0]["offset_line"], 200);
}
