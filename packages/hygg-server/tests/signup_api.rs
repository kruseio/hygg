//! Account signup over the API: `POST /api/v1/signup` creates a user and mints
//! its first device token in one call. A duplicate email is a conflict, a
//! too-short password a bad request, and the returned token authenticates
//! against `/me` straight away from its binding machine.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::response::Response;
use http_body_util::BodyExt;
use hygg_server::bootstrap::ensure_default_tenant;
use hygg_server::config::Config;
use hygg_server::db::Db;
use hygg_server::state::AppState;
use hygg_server::{app, repo};
use serde_json::{Value, json};
use tower::ServiceExt;

async fn setup() -> (tempfile::TempDir, AppState) {
  let dir = tempfile::tempdir().unwrap();
  let url = format!("sqlite://{}", dir.path().join("t.db").display());
  let db = Db::connect(&url).await.unwrap();
  db.migrate().await.unwrap();
  let state = AppState::new(db, Config::from_env());
  ensure_default_tenant(&state).await.unwrap();
  (dir, state)
}

async fn json_body(resp: Response) -> Value {
  let bytes = resp.into_body().collect().await.unwrap().to_bytes();
  serde_json::from_slice(&bytes).unwrap()
}

fn signup_req(email: &str, password: &str) -> Request<Body> {
  Request::builder()
    .method("POST")
    .uri("/api/v1/signup")
    .header(header::CONTENT_TYPE, "application/json")
    .body(Body::from(
      json!({"email": email, "password": password, "machine_id": "m1"})
        .to_string(),
    ))
    .unwrap()
}

#[tokio::test]
async fn signup_creates_account_and_returns_working_token() {
  let (_dir, state) = setup().await;
  let resp = app(state.clone())
    .oneshot(signup_req("new@x.y", "password123"))
    .await
    .unwrap();
  assert_eq!(resp.status(), StatusCode::OK);
  let body = json_body(resp).await;
  let token = body["token"].as_str().unwrap().to_string();
  assert!(!token.is_empty());
  assert!(!body["device_id"].as_str().unwrap().is_empty());
  assert!(!body["user_id"].as_str().unwrap().is_empty());

  // The freshly issued token authenticates from the machine it was bound to.
  let me = Request::builder()
    .method("GET")
    .uri("/api/v1/me")
    .header(header::AUTHORIZATION, format!("Bearer {token}"))
    .header("x-hygg-user", "new@x.y")
    .header("x-hygg-machine-id", "m1")
    .body(Body::empty())
    .unwrap();
  let resp = app(state.clone()).oneshot(me).await.unwrap();
  assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn duplicate_email_is_conflict() {
  let (_dir, state) = setup().await;
  let first = app(state.clone())
    .oneshot(signup_req("dup@x.y", "password123"))
    .await
    .unwrap();
  assert_eq!(first.status(), StatusCode::OK);
  let again = app(state.clone())
    .oneshot(signup_req("dup@x.y", "password123"))
    .await
    .unwrap();
  assert_eq!(again.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn too_short_password_is_bad_request() {
  let (_dir, state) = setup().await;
  let resp = app(state).oneshot(signup_req("weak@x.y", "short")).await.unwrap();
  assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}
