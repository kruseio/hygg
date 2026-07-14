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

/// The setup user's email and a stable machine id that every authenticated
/// request in this suite carries (bearer auth now also requires the username
/// and a machine id; a device binds to the first machine it is seen with).
pub(crate) const USER_EMAIL: &str = "u@x.y";
pub(crate) const MACHINE: &str = "test-machine";

pub(crate) async fn setup() -> (tempfile::TempDir, AppState) {
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

pub(crate) async fn json_body(resp: Response) -> Value {
  let bytes = resp.into_body().collect().await.unwrap().to_bytes();
  serde_json::from_slice(&bytes).unwrap()
}

/// Register a device and return its bearer token.
pub(crate) async fn register_device(state: &AppState) -> String {
  let req = Request::builder()
    .method("POST")
    .uri("/api/v1/devices/register")
    .header(header::CONTENT_TYPE, "application/json")
    .body(Body::from(json!({"email":"u@x.y","password":"pw"}).to_string()))
    .unwrap();
  let resp = app(state.clone()).oneshot(req).await.unwrap();
  json_body(resp).await["token"].as_str().unwrap().to_string()
}

pub(crate) async fn push(
  state: &AppState,
  token: &str,
  body: Value,
) -> Response {
  let req = Request::builder()
    .method("POST")
    .uri("/api/v1/sync/push")
    .header(header::CONTENT_TYPE, "application/json")
    .header(header::AUTHORIZATION, format!("Bearer {token}"))
    .header("x-hygg-user", USER_EMAIL)
    .header("x-hygg-machine-id", MACHINE)
    .body(Body::from(body.to_string()))
    .unwrap();
  app(state.clone()).oneshot(req).await.unwrap()
}

pub(crate) async fn pull(state: &AppState, token: &str, since: i64) -> Value {
  let req = Request::builder()
    .uri(format!("/api/v1/sync/pull?since={since}"))
    .header(header::AUTHORIZATION, format!("Bearer {token}"))
    .header("x-hygg-user", USER_EMAIL)
    .header("x-hygg-machine-id", MACHINE)
    .body(Body::empty())
    .unwrap();
  json_body(app(state.clone()).oneshot(req).await.unwrap()).await
}

pub(crate) fn progress_op(op_id: &str, offset: i64, updated_at: i64) -> Value {
  json!({
    "op_id": op_id,
    "kind": "progress",
    "book_id": "book-1",
    "updated_at": updated_at,
    "data": { "offset": offset, "total_lines": 900, "percentage": 10.0 }
  })
}

pub(crate) fn progress_op_for(
  op_id: &str,
  book: &str,
  offset: i64,
  at: i64,
) -> Value {
  json!({
    "op_id": op_id, "kind": "progress", "book_id": book, "updated_at": at,
    "data": { "offset": offset, "total_lines": 900, "percentage": 10.0 }
  })
}
