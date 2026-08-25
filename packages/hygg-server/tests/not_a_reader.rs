//! The server is a sync backend, not a reader.
//!
//! Document bytes are handed to a client to render; the server never serves a
//! document as something a browser would display. This test pins that at the
//! HTTP layer: a blob download is an `attachment` with sniffing disabled, so
//! pointing a browser at a blob URL downloads bytes rather than reading a
//! document.

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
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tower::ServiceExt;

const EMAIL: &str = "r@x.y";
const HASH: &str = "reader-hash-1";

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
    "R",
    Some(&hash),
    "user",
  )
  .await
  .unwrap();
  (dir, state)
}

async fn send(state: &AppState, req: Request<Body>) -> Response {
  app(state.clone()).oneshot(req).await.unwrap()
}

async fn json_body<T: DeserializeOwned>(resp: Response) -> T {
  let bytes = resp.into_body().collect().await.unwrap().to_bytes();
  serde_json::from_slice(&bytes).unwrap()
}

async fn register(state: &AppState) -> String {
  let req = Request::builder()
    .method("POST")
    .uri("/api/v1/devices/register")
    .header(header::CONTENT_TYPE, "application/json")
    .body(Body::from(json!({"email": EMAIL, "password": "pw"}).to_string()))
    .unwrap();
  json_body::<Value>(send(state, req).await).await["token"]
    .as_str()
    .unwrap()
    .to_string()
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
    .header("x-hygg-machine-id", "reader-machine")
}

#[tokio::test]
async fn a_blob_download_is_never_rendered_inline() {
  let (_d, state) = setup().await;
  let token = register(&state).await;

  // Register a document and upload its bytes (HTML, the format a browser would
  // most eagerly render inline if allowed to).
  let up = authed("POST", "/api/v1/books", &token)
    .header(header::CONTENT_TYPE, "application/json")
    .body(Body::from(
      json!({ "content_hash": HASH, "title": "T", "format": "html",
              "size_bytes": 5 })
      .to_string(),
    ))
    .unwrap();
  assert_eq!(send(&state, up).await.status(), StatusCode::OK);
  let put = authed("PUT", &format!("/api/v1/books/{HASH}/blob"), &token)
    .header(header::CONTENT_TYPE, "application/octet-stream")
    .body(Body::from(b"<h1>x".to_vec()))
    .unwrap();
  assert_eq!(send(&state, put).await.status(), StatusCode::OK);

  // Download it and inspect the headers: the server refuses to be a reader.
  let get = authed("GET", &format!("/api/v1/books/{HASH}/blob"), &token)
    .body(Body::empty())
    .unwrap();
  let resp = send(&state, get).await;
  assert_eq!(resp.status(), StatusCode::OK);
  let headers = resp.headers();
  assert_eq!(
    headers.get(header::CONTENT_TYPE).unwrap(),
    "application/octet-stream"
  );
  assert_eq!(headers.get(header::CONTENT_DISPOSITION).unwrap(), "attachment");
  assert_eq!(headers.get(header::X_CONTENT_TYPE_OPTIONS).unwrap(), "nosniff");
}
