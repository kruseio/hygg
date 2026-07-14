use axum::body::Body;
use axum::http::StatusCode;
use hygg_server::app;
use serde_json::json;
use tower::ServiceExt;

use crate::helpers::*;

#[tokio::test]
async fn sync_mode_defaults_full_and_can_be_set() {
  let (_dir, state, token) = setup().await;
  let body = json!({"content_hash":"hashA","title":"Dune","format":"epub"});
  app(state.clone())
    .oneshot(authed(
      "POST",
      "/api/v1/books",
      &token,
      Body::from(body.to_string()),
    ))
    .await
    .unwrap();

  // A fresh document defaults to full sync.
  let list = json_body(
    app(state.clone())
      .oneshot(authed("GET", "/api/v1/books", &token, Body::empty()))
      .await
      .unwrap(),
  )
  .await;
  assert_eq!(list[0]["sync_mode"], "full");

  // The account-wide ceiling can be lowered, and the change is visible in the
  // list.
  let resp = app(state.clone())
    .oneshot(authed(
      "PUT",
      "/api/v1/books/hashA/sync-mode",
      &token,
      Body::from(json!({"sync_mode":"metadata"}).to_string()),
    ))
    .await
    .unwrap();
  assert_eq!(resp.status(), StatusCode::OK);
  let list = json_body(
    app(state.clone())
      .oneshot(authed("GET", "/api/v1/books", &token, Body::empty()))
      .await
      .unwrap(),
  )
  .await;
  assert_eq!(list[0]["sync_mode"], "metadata");
}

#[tokio::test]
async fn metadata_mode_rejects_blob_upload() {
  let (_dir, state, token) = setup().await;
  let body = json!({"content_hash":"hashA","title":"Dune","format":"epub"});
  app(state.clone())
    .oneshot(authed(
      "POST",
      "/api/v1/books",
      &token,
      Body::from(body.to_string()),
    ))
    .await
    .unwrap();
  app(state.clone())
    .oneshot(authed(
      "PUT",
      "/api/v1/books/hashA/sync-mode",
      &token,
      Body::from(json!({"sync_mode":"metadata"}).to_string()),
    ))
    .await
    .unwrap();

  // The server refuses the bytes under a metadata-only ceiling.
  let resp = app(state.clone())
    .oneshot(authed(
      "PUT",
      "/api/v1/books/hashA/blob",
      &token,
      Body::from(vec![1u8, 2, 3]),
    ))
    .await
    .unwrap();
  assert_eq!(resp.status(), StatusCode::CONFLICT);

  // Raising the ceiling back to full re-enables the upload.
  app(state.clone())
    .oneshot(authed(
      "PUT",
      "/api/v1/books/hashA/sync-mode",
      &token,
      Body::from(json!({"sync_mode":"full"}).to_string()),
    ))
    .await
    .unwrap();
  let resp = app(state.clone())
    .oneshot(authed(
      "PUT",
      "/api/v1/books/hashA/blob",
      &token,
      Body::from(vec![1u8, 2, 3]),
    ))
    .await
    .unwrap();
  assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn off_mode_skips_progress_ops() {
  let (_dir, state, token) = setup().await;
  let body = json!({"content_hash":"hashA","title":"Dune","format":"epub"});
  app(state.clone())
    .oneshot(authed(
      "POST",
      "/api/v1/books",
      &token,
      Body::from(body.to_string()),
    ))
    .await
    .unwrap();
  app(state.clone())
    .oneshot(authed(
      "PUT",
      "/api/v1/books/hashA/sync-mode",
      &token,
      Body::from(json!({"sync_mode":"off"}).to_string()),
    ))
    .await
    .unwrap();

  // A progress op for an off document is accepted by the endpoint but dropped
  // (skipped, not applied) — the server is authoritative even if a client
  // tries.
  let op = json!({
    "op_id":"p1","kind":"progress","book_id":"hashA","updated_at":1000,
    "data":{"offset":5,"total_lines":900,"percentage":1.0}
  });
  let resp = json_body(
    app(state.clone())
      .oneshot(authed(
        "POST",
        "/api/v1/sync/push",
        &token,
        Body::from(json!({"ops":[op]}).to_string()),
      ))
      .await
      .unwrap(),
  )
  .await;
  assert_eq!(resp["applied"].as_array().unwrap().len(), 0);
  assert_eq!(resp["skipped"][0], "p1");

  // And nothing is returned on pull.
  let pulled = json_body(
    app(state.clone())
      .oneshot(authed(
        "GET",
        "/api/v1/sync/pull?since=0",
        &token,
        Body::empty(),
      ))
      .await
      .unwrap(),
  )
  .await;
  assert_eq!(pulled["progress"].as_array().unwrap().len(), 0);
}
