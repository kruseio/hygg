use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::response::Response;
use http_body_util::BodyExt;
use hygg_server::auth::password::hash_password;
use hygg_server::auth::token::generate_token;
use hygg_server::bootstrap::ensure_default_tenant;
use hygg_server::config::Config;
use hygg_server::db::Db;
use hygg_server::state::AppState;
use hygg_server::{app, repo};
use serde_json::{Value, json};
use tower::ServiceExt;

/// A migrated DB with one tenant and one user (password `hunter2`).
async fn setup() -> (tempfile::TempDir, AppState) {
  let dir = tempfile::tempdir().unwrap();
  let url = format!("sqlite://{}", dir.path().join("t.db").display());
  let db = Db::connect(&url).await.unwrap();
  db.migrate().await.unwrap();
  let state = AppState::new(db, Config::from_env());
  let tenant_id = ensure_default_tenant(&state).await.unwrap();
  let hash = hash_password("hunter2").unwrap();
  repo::users::insert(
    &state.db.conn,
    &tenant_id,
    "a@b.c",
    "Alice",
    Some(&hash),
    "user",
  )
  .await
  .unwrap();
  (dir, state)
}

async fn body_json(resp: Response) -> Value {
  let bytes = resp.into_body().collect().await.unwrap().to_bytes();
  serde_json::from_slice(&bytes).unwrap()
}

fn register_body(email: &str, password: &str) -> Body {
  Body::from(
    json!({ "email": email, "password": password, "device_name": "laptop" })
      .to_string(),
  )
}

async fn register(state: &AppState, email: &str, password: &str) -> Response {
  let req = Request::builder()
    .method("POST")
    .uri("/api/v1/devices/register")
    .header(header::CONTENT_TYPE, "application/json")
    .body(register_body(email, password))
    .unwrap();
  app(state.clone()).oneshot(req).await.unwrap()
}

#[tokio::test]
async fn register_then_me_roundtrip() {
  let (_dir, state) = setup().await;

  let resp = register(&state, "a@b.c", "hunter2").await;
  assert_eq!(resp.status(), StatusCode::OK);
  let registered = body_json(resp).await;
  let token = registered["token"].as_str().unwrap().to_string();
  assert!(token.contains('.'), "token is prefix.secret");

  let req = Request::builder()
    .uri("/api/v1/me")
    .header(header::AUTHORIZATION, format!("Bearer {token}"))
    .header("x-hygg-user", "a@b.c")
    .header("x-hygg-machine-id", "test-machine")
    .body(Body::empty())
    .unwrap();
  let resp = app(state.clone()).oneshot(req).await.unwrap();
  assert_eq!(resp.status(), StatusCode::OK);
  let me = body_json(resp).await;
  assert_eq!(me["is_admin"], false);
  // No deployment label unless an override supplies one.
  assert_eq!(me["label"], serde_json::Value::Null);
  assert_eq!(me["default_access"], "read_write");
  assert_eq!(me["read_only"], false);
  assert_eq!(me["device_id"], registered["device_id"]);
}

#[tokio::test]
async fn register_with_wrong_password_is_unauthorized() {
  let (_dir, state) = setup().await;
  let resp = register(&state, "a@b.c", "wrong").await;
  assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn me_without_token_is_unauthorized() {
  let (_dir, state) = setup().await;
  let req = Request::builder().uri("/api/v1/me").body(Body::empty()).unwrap();
  let resp = app(state).oneshot(req).await.unwrap();
  assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn me_with_bogus_token_is_unauthorized() {
  let (_dir, state) = setup().await;
  let req = Request::builder()
    .uri("/api/v1/me")
    .header(header::AUTHORIZATION, "Bearer deadbeef.totally-wrong")
    .body(Body::empty())
    .unwrap();
  let resp = app(state).oneshot(req).await.unwrap();
  assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// Tests for a gated account live with whatever override does the gating; this
// server admits any authenticated user.
