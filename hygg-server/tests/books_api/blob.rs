use axum::body::Body;
use axum::http::StatusCode;
use http_body_util::BodyExt;
use hygg_server::app;
use serde_json::json;
use tower::ServiceExt;

use crate::helpers::*;

#[tokio::test]
async fn upsert_list_upload_download_roundtrip() {
  let (_dir, state, token) = setup().await;

  // Register metadata.
  let body = json!({"content_hash":"hashA","title":"Dune","format":"epub","size_bytes":1234});
  let resp = app(state.clone())
    .oneshot(authed(
      "POST",
      "/api/v1/books",
      &token,
      Body::from(body.to_string()),
    ))
    .await
    .unwrap();
  assert_eq!(resp.status(), StatusCode::OK);

  // It appears in the list.
  let list = json_body(
    app(state.clone())
      .oneshot(authed("GET", "/api/v1/books", &token, Body::empty()))
      .await
      .unwrap(),
  )
  .await;
  let books = list.as_array().unwrap();
  assert_eq!(books.len(), 1);
  assert_eq!(books[0]["content_hash"], "hashA");
  assert_eq!(books[0]["title"], "Dune");

  // Upload bytes.
  let resp = app(state.clone())
    .oneshot(authed(
      "PUT",
      "/api/v1/books/hashA/blob",
      &token,
      Body::from(vec![1u8, 2, 3, 4, 5]),
    ))
    .await
    .unwrap();
  assert_eq!(resp.status(), StatusCode::OK);
  assert_eq!(json_body(resp).await["byte_len"], 5);

  // Download identical bytes.
  let resp = app(state.clone())
    .oneshot(authed("GET", "/api/v1/books/hashA/blob", &token, Body::empty()))
    .await
    .unwrap();
  assert_eq!(resp.status(), StatusCode::OK);
  let bytes = resp.into_body().collect().await.unwrap().to_bytes();
  assert_eq!(bytes.as_ref(), &[1u8, 2, 3, 4, 5]);
}

#[tokio::test]
async fn download_unknown_book_is_404() {
  let (_dir, state, token) = setup().await;
  let resp = app(state)
    .oneshot(authed("GET", "/api/v1/books/missing/blob", &token, Body::empty()))
    .await
    .unwrap();
  assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn oversized_body_is_rejected() {
  let (_dir, mut state, token) = setup().await;
  // Tighten the limit so the test payload trips it without huge allocations.
  state.config.max_body_bytes = 16;
  let resp = app(state.clone())
    .oneshot(authed(
      "PUT",
      "/api/v1/books/hashA/blob",
      &token,
      Body::from(vec![0u8; 1024]),
    ))
    .await
    .unwrap();
  assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
}
