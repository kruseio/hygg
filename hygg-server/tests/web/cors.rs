//! CORS behaviour for the browser PWA (separate origin, bearer-token API).
//! Each test pins `cors_allow_origins` on the state explicitly, so the suite
//! is hermetic — it never depends on `CORS_ALLOW_ORIGINS` in the process
//! environment. (That the unset default resolves to `*` is covered by the
//! config unit tests.)

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use hygg_server::state::AppState;
use tower::ServiceExt;

use crate::helpers::*;

const PWA: &str = "https://pwa.hygg.kruseio.com";

/// A migrated state with an explicit CORS allow-list (`["*"]` = wildcard).
async fn state_with_origins(origins: &[&str]) -> (tempfile::TempDir, AppState) {
  let (dir, mut state) = migrated_state().await;
  state.config.cors_allow_origins =
    origins.iter().map(|origin| origin.to_string()).collect();
  (dir, state)
}

/// The locked-down deployment: PWA origin + local dev (what a deployment
/// configures by default).
async fn allow_listed_state() -> (tempfile::TempDir, AppState) {
  state_with_origins(&[PWA, "http://localhost:8080", "http://127.0.0.1:8080"])
    .await
}

#[tokio::test]
async fn simple_request_from_listed_origin_echoes_it() {
  let (_dir, state) = allow_listed_state().await;
  let response = hygg_server::app(state)
    .oneshot(
      Request::builder()
        .uri("/health")
        .header(header::ORIGIN, PWA)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::OK);
  assert_eq!(
    response
      .headers()
      .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
      .and_then(|v| v.to_str().ok()),
    Some(PWA)
  );
}

#[tokio::test]
async fn preflight_allows_bearer_json_api() {
  let (_dir, state) = allow_listed_state().await;
  let response = hygg_server::app(state)
    .oneshot(
      Request::builder()
        .method(Method::OPTIONS)
        .uri("/api/v1/sync/push")
        .header(header::ORIGIN, PWA)
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
        .header(
          header::ACCESS_CONTROL_REQUEST_HEADERS,
          "authorization,x-hygg-user,x-hygg-machine-id",
        )
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();

  assert!(response.status().is_success());
  let headers = response.headers();
  assert_eq!(
    headers
      .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
      .and_then(|v| v.to_str().ok()),
    Some(PWA)
  );
  let methods = headers
    .get(header::ACCESS_CONTROL_ALLOW_METHODS)
    .and_then(|v| v.to_str().ok())
    .unwrap_or_default();
  assert!(methods.contains("POST"), "methods: {methods}");
  let allow_headers = headers
    .get(header::ACCESS_CONTROL_ALLOW_HEADERS)
    .and_then(|v| v.to_str().ok())
    .unwrap_or_default()
    .to_ascii_lowercase();
  assert!(allow_headers.contains("authorization"), "headers: {allow_headers}");
  // The PWA is cross-origin and sends the username + machine-id headers on
  // every request; they must be admitted or the browser preflight blocks the
  // call.
  assert!(allow_headers.contains("x-hygg-user"), "headers: {allow_headers}");
  assert!(
    allow_headers.contains("x-hygg-machine-id"),
    "headers: {allow_headers}"
  );
}

#[tokio::test]
async fn wildcard_default_admits_lan_dev_origins() {
  // The self-host default resolves to "*", so a PWA served from any LAN/dev
  // address reaches a local server without configuration.
  let (_dir, state) = state_with_origins(&["*"]).await;
  let response = hygg_server::app(state)
    .oneshot(
      Request::builder()
        .uri("/health")
        .header(header::ORIGIN, "http://127.0.0.1:8080")
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(
    response
      .headers()
      .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
      .and_then(|v| v.to_str().ok()),
    Some("*")
  );
}

#[tokio::test]
async fn unlisted_origin_is_not_allowed() {
  let (_dir, state) = allow_listed_state().await;
  let response = hygg_server::app(state)
    .oneshot(
      Request::builder()
        .uri("/health")
        .header(header::ORIGIN, "https://evil.example.com")
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::OK);
  assert!(
    response.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN).is_none(),
    "an unlisted origin must not receive an allow-origin header"
  );
}
