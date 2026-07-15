//! Device self-management: a user can list their devices and revoke one,
//! which immediately invalidates that device's token while leaving others
//! working. Revoking an unknown device is a 404.

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
use serde_json::{Value, json};
use tower::ServiceExt;

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
    "u@x.y",
    "U",
    Some(&hash),
    "user",
  )
  .await
  .unwrap();
  (dir, state)
}

async fn json_body(resp: Response) -> Value {
  let bytes = resp.into_body().collect().await.unwrap().to_bytes();
  serde_json::from_slice(&bytes).unwrap()
}

/// Register a device; returns `(device_id, token)`.
async fn register_device(state: &AppState) -> (String, String) {
  let req = Request::builder()
    .method("POST")
    .uri("/api/v1/devices/register")
    .header(header::CONTENT_TYPE, "application/json")
    .body(Body::from(json!({"email":"u@x.y","password":"pw"}).to_string()))
    .unwrap();
  let body = json_body(app(state.clone()).oneshot(req).await.unwrap()).await;
  (
    body["device_id"].as_str().unwrap().to_string(),
    body["token"].as_str().unwrap().to_string(),
  )
}

fn authed(method: &str, uri: &str, token: &str) -> Request<Body> {
  Request::builder()
    .method(method)
    .uri(uri)
    .header(header::AUTHORIZATION, format!("Bearer {token}"))
    .header("x-hygg-user", "u@x.y")
    .header("x-hygg-machine-id", "test-machine")
    .body(Body::empty())
    .unwrap()
}

async fn status(state: &AppState, req: Request<Body>) -> StatusCode {
  app(state.clone()).oneshot(req).await.unwrap().status()
}

#[tokio::test]
async fn list_then_revoke_invalidates_only_that_device() {
  let (_dir, state) = setup().await;
  let (id_a, token_a) = register_device(&state).await;
  let (_id_b, token_b) = register_device(&state).await;

  // Both devices are listed.
  let list = json_body(
    app(state.clone())
      .oneshot(authed("GET", "/api/v1/devices", &token_a))
      .await
      .unwrap(),
  )
  .await;
  assert_eq!(list.as_array().unwrap().len(), 2);

  // Revoke device A.
  let resp = app(state.clone())
    .oneshot(authed("DELETE", &format!("/api/v1/devices/{id_a}"), &token_b))
    .await
    .unwrap();
  assert_eq!(resp.status(), StatusCode::OK);

  // A's token no longer authenticates; B's still does.
  assert_eq!(
    status(&state, authed("GET", "/api/v1/me", &token_a)).await,
    StatusCode::UNAUTHORIZED
  );
  assert_eq!(
    status(&state, authed("GET", "/api/v1/me", &token_b)).await,
    StatusCode::OK
  );
}

#[tokio::test]
async fn revoking_unknown_device_is_404() {
  let (_dir, state) = setup().await;
  let (_id, token) = register_device(&state).await;
  assert_eq!(
    status(&state, authed("DELETE", "/api/v1/devices/nope", &token)).await,
    StatusCode::NOT_FOUND
  );
}

// Device caps are enforced by whatever implements the entitlements hook,
// and the open self-host core registers devices without any cap.
