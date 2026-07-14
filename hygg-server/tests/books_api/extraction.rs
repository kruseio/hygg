use axum::body::Body;
use axum::http::StatusCode;
use hygg_server::{app, repo};
use serde_json::json;
use tower::ServiceExt;

use crate::helpers::*;
use hygg_server::entity::tenants;
use sea_orm::EntityTrait;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn blob_upload_prewarms_extraction_cache() {
  let (_dir, state, token) = setup().await;

  // Register a text document, then upload its bytes.
  let body = json!({"content_hash":"warm","title":"Note","format":"txt","size_bytes":64});
  app(state.clone())
    .oneshot(authed(
      "POST",
      "/api/v1/books",
      &token,
      Body::from(body.to_string()),
    ))
    .await
    .unwrap();
  let text = b"Plenty of readable words here so extraction yields real text.";
  let resp = app(state.clone())
    .oneshot(authed(
      "PUT",
      "/api/v1/books/warm/blob",
      &token,
      Body::from(text.to_vec()),
    ))
    .await
    .unwrap();
  assert_eq!(resp.status(), StatusCode::OK);

  // The background pre-warm caches the extraction under the book's content hash
  // at the default width. Poll (bounded) for it to land.
  let tenant =
    tenants::Entity::find().one(&state.db.conn).await.unwrap().unwrap();
  let mut cached = None;
  for _ in 0..100 {
    cached = repo::extractions::get(
      &state.db.conn,
      &tenant.id,
      "warm",
      hygg_server::api::convert::EXTRACTOR_VERSION,
      hygg_server::api::convert::PREWARM_COL as i64,
    )
    .await
    .unwrap();
    if cached.is_some() {
      break;
    }
    tokio::task::spawn_blocking(|| {
      std::thread::sleep(std::time::Duration::from_millis(20))
    })
    .await
    .unwrap();
  }
  let cached =
    cached.expect("blob upload should pre-warm the extraction cache");
  assert_eq!(cached.format, "txt");
  assert!(cached.text.contains("readable"), "got: {}", cached.text);
}

#[tokio::test]
async fn extraction_endpoint_returns_server_text_for_stored_doc() {
  let (_dir, state, token) = setup().await;
  let body = json!({"content_hash":"exhash","title":"Notes","format":"txt","size_bytes":80});
  app(state.clone())
    .oneshot(authed(
      "POST",
      "/api/v1/books",
      &token,
      Body::from(body.to_string()),
    ))
    .await
    .unwrap();
  let text = b"Enough readable words here that the extraction of the stored \
    document returns real justified text content for the browser to render.";
  app(state.clone())
    .oneshot(authed(
      "PUT",
      "/api/v1/books/exhash/blob",
      &token,
      Body::from(text.to_vec()),
    ))
    .await
    .unwrap();

  // The extraction endpoint returns justified server text for the stored doc,
  // without the client re-uploading the bytes.
  let resp = app(state.clone())
    .oneshot(authed(
      "GET",
      "/api/v1/books/exhash/extraction?col=40",
      &token,
      Body::empty(),
    ))
    .await
    .unwrap();
  assert_eq!(resp.status(), StatusCode::OK);
  let out = json_body(resp).await;
  assert_eq!(out["format"], "txt");
  assert!(out["text"].as_str().unwrap().contains("readable"));
}

#[tokio::test]
async fn extraction_of_metadata_only_doc_is_404() {
  let (_dir, state, token) = setup().await;
  let body = json!({"content_hash":"nob","title":"N","format":"txt"});
  app(state.clone())
    .oneshot(authed(
      "POST",
      "/api/v1/books",
      &token,
      Body::from(body.to_string()),
    ))
    .await
    .unwrap();
  // No blob uploaded, so there is nothing to extract.
  let resp = app(state.clone())
    .oneshot(authed(
      "GET",
      "/api/v1/books/nob/extraction",
      &token,
      Body::empty(),
    ))
    .await
    .unwrap();
  assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
