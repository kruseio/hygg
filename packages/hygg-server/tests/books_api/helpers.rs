use axum::body::Body;
use axum::http::{Request, header};
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

pub(crate) async fn json_body(resp: Response) -> Value {
  let bytes = resp.into_body().collect().await.unwrap().to_bytes();
  serde_json::from_slice(&bytes).unwrap()
}

/// Migrated DB + a registered device token.
pub(crate) async fn setup() -> (tempfile::TempDir, AppState, String) {
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
  let req = Request::builder()
    .method("POST")
    .uri("/api/v1/devices/register")
    .header(header::CONTENT_TYPE, "application/json")
    .body(Body::from(json!({"email":"u@x.y","password":"pw"}).to_string()))
    .unwrap();
  let resp = app(state.clone()).oneshot(req).await.unwrap();
  let token = json_body(resp).await["token"].as_str().unwrap().to_string();
  (dir, state, token)
}

pub(crate) fn authed(
  method: &str,
  uri: &str,
  token: &str,
  body: Body,
) -> Request<Body> {
  Request::builder()
    .method(method)
    .uri(uri)
    .header(header::AUTHORIZATION, format!("Bearer {token}"))
    .header("x-hygg-user", "u@x.y")
    .header("x-hygg-machine-id", "test-machine")
    .header(header::CONTENT_TYPE, "application/json")
    .body(body)
    .unwrap()
}
