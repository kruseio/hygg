//! Full-account export/import round-trips: a user's library, document bytes,
//! tags, and annotations exported from one server import cleanly into a fresh
//! server (the migration path between deployments).

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::response::Response;
use http_body_util::BodyExt;
use hygg_server::auth::password::hash_password;
use hygg_server::bootstrap::ensure_default_tenant;
use hygg_server::config::Config;
use hygg_server::db::Db;
use hygg_server::state::AppState;
use hygg_server::{app, repo};
use hygg_shared::export::{ExportBundle, ImportSummary};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tower::ServiceExt;

const EMAIL: &str = "u@x.y";
const MACHINE: &str = "export-machine";
const HASH: &str = "book-hash-1";
const BLOB: &[u8] = b"hello export world";

/// A fresh server with the `u@x.y` user seeded.
async fn setup() -> (tempfile::TempDir, AppState) {
  let dir = tempfile::tempdir().unwrap();
  let url = format!("sqlite://{}", dir.path().join("t.db").display());
  let db = Db::connect(&url).await.unwrap();
  db.migrate().await.unwrap();
  let state = AppState::new(db, Config::from_env());
  let tenant_id = ensure_default_tenant(&state).await.unwrap();
  let hash = hash_password("pw").unwrap();
  repo::users::insert(
    &state.db.conn,
    &tenant_id,
    EMAIL,
    "U",
    Some(&hash),
    "user",
  )
  .await
  .unwrap();
  (dir, state)
}

async fn register(state: &AppState) -> String {
  let req = Request::builder()
    .method("POST")
    .uri("/api/v1/devices/register")
    .header(header::CONTENT_TYPE, "application/json")
    .body(Body::from(json!({"email": EMAIL, "password": "pw"}).to_string()))
    .unwrap();
  let resp = app(state.clone()).oneshot(req).await.unwrap();
  assert_eq!(resp.status(), StatusCode::OK);
  json_body::<Value>(resp).await["token"].as_str().unwrap().to_string()
}

fn authed(
  method: &str,
  uri: &str,
  token: &str,
) -> axum::http::request::Builder {
  Request::builder()
    .method(method)
    .uri(uri)
    .header(header::AUTHORIZATION, format!("Bearer {token}"))
    .header("x-hygg-user", EMAIL)
    .header("x-hygg-machine-id", MACHINE)
}

async fn send(state: &AppState, req: Request<Body>) -> Response {
  app(state.clone()).oneshot(req).await.unwrap()
}

async fn json_body<T: DeserializeOwned>(resp: Response) -> T {
  let bytes = resp.into_body().collect().await.unwrap().to_bytes();
  serde_json::from_slice(&bytes).unwrap()
}

/// Upload a document + its blob and push one of each annotation kind.
async fn seed_library(state: &AppState, token: &str) {
  let upsert = authed("POST", "/api/v1/books", token)
    .header(header::CONTENT_TYPE, "application/json")
    .body(Body::from(
      json!({
        "content_hash": HASH, "title": "The Title", "author": "The Author",
        "format": "pdf", "size_bytes": BLOB.len()
      })
      .to_string(),
    ))
    .unwrap();
  assert_eq!(send(state, upsert).await.status(), StatusCode::OK);

  let put = authed("PUT", &format!("/api/v1/books/{HASH}/blob"), token)
    .header(header::CONTENT_TYPE, "application/octet-stream")
    .body(Body::from(BLOB))
    .unwrap();
  assert_eq!(send(state, put).await.status(), StatusCode::OK);

  let ops = json!({ "ops": [
    { "op_id": "op-p", "kind": "progress", "book_id": HASH, "updated_at": 100,
      "data": { "offset": 12, "total_lines": 900, "percentage": 1.3, "word_offset": 7 } },
    { "op_id": "op-b", "kind": "bookmark", "book_id": HASH, "updated_at": 101,
      "data": { "mark": "a", "line": 5, "col": 2 } },
    { "op_id": "op-h", "kind": "highlight", "book_id": HASH, "updated_at": 102,
      "data": { "start_offset": 3, "end_offset": 9 } },
    { "op_id": "op-n", "kind": "note", "book_id": HASH, "updated_at": 103,
      "data": { "id": "note-1", "body": "a thought", "line": 4 } }
  ]});
  let push = authed("POST", "/api/v1/sync/push", token)
    .header(header::CONTENT_TYPE, "application/json")
    .body(Body::from(ops.to_string()))
    .unwrap();
  let resp = send(state, push).await;
  assert_eq!(resp.status(), StatusCode::OK);
  let applied = json_body::<Value>(resp).await;
  assert_eq!(applied["applied"].as_array().unwrap().len(), 4);
}

async fn export(state: &AppState, token: &str) -> ExportBundle {
  let req = authed("GET", "/api/v1/export", token).body(Body::empty()).unwrap();
  let resp = send(state, req).await;
  assert_eq!(resp.status(), StatusCode::OK);
  json_body(resp).await
}

#[tokio::test]
async fn export_then_import_into_fresh_server_reproduces_the_library() {
  // Source server: seed a library and export it.
  let (_dir_a, server_a) = setup().await;
  let token_a = register(&server_a).await;
  seed_library(&server_a, &token_a).await;
  let bundle = export(&server_a, &token_a).await;

  // The bundle carries the one document with its blob and every annotation.
  assert_eq!(bundle.books.len(), 1);
  let book = &bundle.books[0];
  assert_eq!(book.content_hash, HASH);
  assert_eq!(book.title, "The Title");
  assert_eq!(book.author, "The Author");
  assert!(book.blob_base64.is_some(), "blob should be included");
  assert_eq!(book.progress.as_ref().unwrap().offset_line, 12);
  assert_eq!(book.progress.as_ref().unwrap().word_offset, Some(7));
  assert_eq!(book.bookmarks.len(), 1);
  assert_eq!(book.highlights.len(), 1);
  assert_eq!(book.notes.len(), 1);
  assert_eq!(book.notes[0].body, "a thought");

  // Destination server: a brand-new, empty server with the same user.
  let (_dir_b, server_b) = setup().await;
  let token_b = register(&server_b).await;

  let import_req = authed("POST", "/api/v1/import", &token_b)
    .header(header::CONTENT_TYPE, "application/json")
    .body(Body::from(serde_json::to_string(&bundle).unwrap()))
    .unwrap();
  let resp = send(&server_b, import_req).await;
  assert_eq!(resp.status(), StatusCode::OK);
  let summary: ImportSummary = json_body(resp).await;
  assert_eq!(
    summary,
    ImportSummary {
      books: 1,
      blobs: 1,
      progress: 1,
      bookmarks: 1,
      highlights: 1,
      notes: 1,
      tags: 0,
    }
  );

  // Re-exporting the destination reproduces the source library byte-for-byte
  // (document bytes, reading position, and every annotation).
  let round_tripped = export(&server_b, &token_b).await;
  assert_eq!(round_tripped.books.len(), 1);
  let restored = &round_tripped.books[0];
  assert_eq!(restored.content_hash, HASH);
  assert_eq!(restored.title, "The Title");
  assert_eq!(restored.blob_base64, book.blob_base64);
  assert_eq!(restored.progress.as_ref().unwrap().offset_line, 12);
  assert_eq!(restored.progress.as_ref().unwrap().word_offset, Some(7));
  assert_eq!(restored.bookmarks.len(), 1);
  assert_eq!(restored.highlights.len(), 1);
  assert_eq!(restored.notes.len(), 1);
  assert_eq!(restored.notes[0].body, "a thought");

  // And the restored blob decodes to the original document bytes.
  let blob = server_blob(&server_b, HASH).await;
  assert_eq!(blob, BLOB);
}

/// Read a stored blob straight from the destination DB to prove the bytes
/// survived the base64 round-trip intact.
async fn server_blob(state: &AppState, content_hash: &str) -> Vec<u8> {
  let tenant = ensure_default_tenant(state).await.unwrap();
  let book_id =
    repo::books::find_id_by_hash(&state.db.conn, &tenant, content_hash)
      .await
      .unwrap()
      .unwrap();
  repo::blobs::get(&state.db.conn, &tenant, &book_id).await.unwrap().unwrap()
}
