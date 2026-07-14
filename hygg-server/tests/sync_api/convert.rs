//! `/api/v1/convert` — server-side document extraction, entitlement-gated.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use hygg_server::app;
use tower::ServiceExt;

use crate::helpers::*;
use hygg_server::entity::tenants;
use sea_orm::EntityTrait;

#[tokio::test]
async fn convert_txt_returns_justified_text() {
  let (_dir, state) = setup().await;
  let token = register_device(&state).await;

  let text = "Hello world, this is a plain text document the server should \
    justify into the hygg monospace column so the browser PWA can render it.";
  let req = Request::builder()
    .method("POST")
    .uri("/api/v1/convert?filename=note.txt&col=40")
    .header(header::AUTHORIZATION, format!("Bearer {token}"))
    .header("x-hygg-user", USER_EMAIL)
    .header("x-hygg-machine-id", MACHINE)
    .header(header::CONTENT_TYPE, "application/octet-stream")
    .body(Body::from(text))
    .unwrap();
  let resp = app(state.clone()).oneshot(req).await.unwrap();

  assert_eq!(resp.status(), StatusCode::OK);
  let body = json_body(resp).await;
  assert_eq!(body["title"], "note");
  assert_eq!(body["format"], "txt");
  let out = body["text"].as_str().unwrap();
  assert!(out.contains("Hello"), "expected justified text, got: {out}");
  // Justified to col=40, so at least one line wraps under the width.
  assert!(out.lines().any(|l| l.chars().count() <= 40));
}

#[tokio::test]
async fn convert_caches_and_reuses_extraction() {
  let (_dir, state) = setup().await;
  let token = register_device(&state).await;

  let text = "The quick brown fox jumps over the lazy dog, and the server \
    should cache this extraction so a second identical request is served from \
    book_extractions instead of re-running the pipeline.";
  let send = || {
    let req = Request::builder()
      .method("POST")
      .uri("/api/v1/convert?filename=note.txt&col=40")
      .header(header::AUTHORIZATION, format!("Bearer {token}"))
      .header("x-hygg-user", USER_EMAIL)
      .header("x-hygg-machine-id", MACHINE)
      .header(header::CONTENT_TYPE, "application/octet-stream")
      .body(Body::from(text))
      .unwrap();
    app(state.clone()).oneshot(req)
  };

  // First call is a miss: it computes and writes the cache row.
  let first = json_body(send().await.unwrap()).await;
  // A row now exists for this content at the default extractor version + width.
  let content_hash = hygg_shared::sync::content_sha256(text.as_bytes());
  let cached = hygg_server::repo::extractions::get(
    &state.db.conn,
    &tenant_id(&state).await,
    &content_hash,
    hygg_server::api::convert::EXTRACTOR_VERSION,
    40,
  )
  .await
  .unwrap();
  assert!(cached.is_some(), "first convert should populate the cache");

  // Second call returns byte-identical output (served from the cache).
  let second = json_body(send().await.unwrap()).await;
  assert_eq!(first["text"], second["text"]);
  assert_eq!(cached.unwrap().text, first["text"].as_str().unwrap());
}

#[tokio::test]
async fn convert_cache_disabled_writes_nothing() {
  // Flip the kill switch on this state directly (rather than via the
  // process-global env var, which would race with parallel tests) so the
  // handler neither reads nor writes the cache.
  let (_dir, mut state) = setup().await;
  state.config.extraction_cache = false;
  let token = register_device(&state).await;

  let text = "No cache should be written when the extraction cache is off.";
  let req = Request::builder()
    .method("POST")
    .uri("/api/v1/convert?filename=note.txt&col=40")
    .header(header::AUTHORIZATION, format!("Bearer {token}"))
    .header("x-hygg-user", USER_EMAIL)
    .header("x-hygg-machine-id", MACHINE)
    .header(header::CONTENT_TYPE, "application/octet-stream")
    .body(Body::from(text))
    .unwrap();
  assert_eq!(
    app(state.clone()).oneshot(req).await.unwrap().status(),
    StatusCode::OK
  );

  let content_hash = hygg_shared::sync::content_sha256(text.as_bytes());
  let cached = hygg_server::repo::extractions::get(
    &state.db.conn,
    &tenant_id(&state).await,
    &content_hash,
    hygg_server::api::convert::EXTRACTOR_VERSION,
    40,
  )
  .await
  .unwrap();
  assert!(cached.is_none(), "cache disabled must not write a row");
}

/// The default tenant's id (tests run against a single bootstrapped tenant).
async fn tenant_id(state: &hygg_server::state::AppState) -> String {
  tenants::Entity::find().one(&state.db.conn).await.unwrap().unwrap().id
}

#[tokio::test]
async fn convert_requires_authentication() {
  let (_dir, state) = setup().await;
  let req = Request::builder()
    .method("POST")
    .uri("/api/v1/convert?filename=note.txt")
    .header(header::CONTENT_TYPE, "application/octet-stream")
    .body(Body::from("hello"))
    .unwrap();
  let resp = app(state).oneshot(req).await.unwrap();
  assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
