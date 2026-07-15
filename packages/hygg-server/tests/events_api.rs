//! SSE push: a device's stream stays open and receives a `changed` event when
//! another device pushes ops for the same user.

use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
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

async fn register(state: &AppState) -> String {
  let req = Request::builder()
    .method("POST")
    .uri("/api/v1/devices/register")
    .header(header::CONTENT_TYPE, "application/json")
    .body(Body::from(json!({"email":"u@x.y","password":"pw"}).to_string()))
    .unwrap();
  let resp = app(state.clone()).oneshot(req).await.unwrap();
  let bytes = resp.into_body().collect().await.unwrap().to_bytes();
  let v: Value = serde_json::from_slice(&bytes).unwrap();
  v["token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn events_requires_auth() {
  let (_dir, state) = setup().await;
  let req =
    Request::builder().uri("/api/v1/events").body(Body::empty()).unwrap();
  let resp = app(state).oneshot(req).await.unwrap();
  assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn push_delivers_a_changed_event_to_a_listener() {
  let (_dir, state) = setup().await;
  let listener_token = register(&state).await;
  let pusher_token = register(&state).await;

  // Open the SSE stream; the handler subscribes before returning the response.
  let sse_req = Request::builder()
    .uri("/api/v1/events")
    .header(header::AUTHORIZATION, format!("Bearer {listener_token}"))
    .header("x-hygg-user", "u@x.y")
    .header("x-hygg-machine-id", "machine-listener")
    .body(Body::empty())
    .unwrap();
  let sse = app(state.clone()).oneshot(sse_req).await.unwrap();
  assert_eq!(sse.status(), StatusCode::OK);
  assert!(
    sse
      .headers()
      .get(header::CONTENT_TYPE)
      .and_then(|v| v.to_str().ok())
      .unwrap_or_default()
      .starts_with("text/event-stream"),
    "SSE content type"
  );
  let mut body = sse.into_body();

  // Another device pushes — this should publish to the listener.
  let op = json!({
    "op_id": "op1", "kind": "progress", "book_id": "b", "updated_at": 1,
    "data": { "offset": 5, "total_lines": 10, "percentage": 50.0 }
  });
  let push = Request::builder()
    .method("POST")
    .uri("/api/v1/sync/push")
    .header(header::AUTHORIZATION, format!("Bearer {pusher_token}"))
    .header("x-hygg-user", "u@x.y")
    .header("x-hygg-machine-id", "machine-pusher")
    .header(header::CONTENT_TYPE, "application/json")
    .body(Body::from(json!({ "ops": [op] }).to_string()))
    .unwrap();
  let push_resp = app(state.clone()).oneshot(push).await.unwrap();
  assert_eq!(push_resp.status(), StatusCode::OK);

  // The listener's stream yields a `changed` event (before any keep-alive).
  let event = read_event(&mut body).await;
  assert!(event.contains("changed"), "expected a changed event, got: {event}");
}

#[tokio::test]
async fn events_authenticates_via_query_params() {
  // The browser client's `EventSource` can't set headers, so the SSE endpoint
  // also accepts the device credential (token + user + machine) in the query
  // string.
  let (_dir, state) = setup().await;
  let token = register(&state).await;
  let uri = format!("/api/v1/events?token={token}&user=u@x.y&machine=m-query");
  let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
  let resp = app(state).oneshot(req).await.unwrap();
  assert_eq!(resp.status(), StatusCode::OK);
  assert!(
    resp
      .headers()
      .get(header::CONTENT_TYPE)
      .and_then(|v| v.to_str().ok())
      .unwrap_or_default()
      .starts_with("text/event-stream"),
    "SSE content type"
  );
}

#[tokio::test]
async fn events_query_auth_rejects_a_bad_token() {
  let (_dir, state) = setup().await;
  let _ = register(&state).await;
  let uri = "/api/v1/events?token=bogus.secret&user=u@x.y&machine=m-query";
  let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
  let resp = app(state).oneshot(req).await.unwrap();
  assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

/// Read SSE frames until one carries event data (ignoring keep-alive pings),
/// failing if nothing arrives promptly.
async fn read_event(body: &mut Body) -> String {
  loop {
    let frame = tokio::time::timeout(Duration::from_secs(5), body.frame())
      .await
      .expect("an SSE frame within the timeout")
      .expect("the stream has not ended")
      .expect("a valid frame");
    if let Some(bytes) = frame.data_ref() {
      let text = String::from_utf8_lossy(bytes).to_string();
      if text.contains("data:") {
        return text;
      }
    }
  }
}
