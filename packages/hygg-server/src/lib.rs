//! hygg-server: a self-hostable, multi-tenant sync server for the hygg reader.
//!
//! The HTTP application is assembled by [`app`] from an [`AppState`], so tests
//! can drive it in-process with `tower`'s `oneshot` (no socket needed) and
//! `main` can serve it over TCP.

pub mod api;
pub mod auth;
pub mod bootstrap;
pub mod config;
pub mod db;
pub mod entity;
pub mod error;
pub mod events;
pub mod ext;
pub mod middleware;
pub mod migration;
pub mod repo;
pub mod runtime;
pub mod state;
pub mod util;
pub mod web;

use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderName, HeaderValue, Method, header};
use axum::{Json, Router, routing::get};
use hygg_shared::sync::headers::{MACHINE_ID_HEADER, USER_HEADER};
use serde_json::json;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;

use config::Config;
use state::AppState;

/// The composable core router: every route this server serves, finalized with
/// `state` so the result is state-erased and can be `merge`d with a downstream
/// router that adds its own. The outer HTTP layers are applied separately by
/// [`layers`].
pub fn routes(state: AppState) -> Router {
  Router::new()
    .route("/health", get(health))
    .merge(web::router())
    .merge(api::router())
    .with_state(state)
}

/// Apply the outer HTTP layers (body-size limit, tracing, CORS) to a fully
/// assembled router. Shared by the binary and any embedder so both get
/// identical body limits and CORS behaviour.
pub fn layers(router: Router, config: &Config) -> Router {
  router
    // Reject oversized requests with 413 before buffering the body.
    .layer(DefaultBodyLimit::max(config.max_body_bytes))
    .layer(TraceLayer::new_for_http())
    // Outermost: answer CORS preflight + tag responses for the browser PWA,
    // which runs on a separate origin and uses bearer tokens (not cookies).
    .layer(build_cors(&config.cors_allow_origins))
}

/// Build the full standalone app: the core [`routes`] plus a minimal root that
/// redirects to the login page, with the outer [`layers`] applied. The
/// composable [`routes`] deliberately omit `/` so an embedder can merge its own
/// page there without a route conflict.
pub fn app(state: AppState) -> Router {
  let config = state.config.clone();
  let root = Router::new()
    .route("/", get(|| async { axum::response::Redirect::to("/login") }));
  layers(routes(state).merge(root), &config)
}

/// CORS allowing the configured PWA origin(s) to call the bearer-token JSON API
/// (`Authorization` + JSON, all the verbs the sync API uses). No credentials —
/// the PWA authenticates with a bearer token, not cookies — so a wildcard `*`
/// origin (the self-host default) is safe: without allow-credentials the
/// browser never sends cookies cross-origin, and an attacker's page still can't
/// obtain the token. A single `*` entry allows any origin; otherwise the exact
/// list is used.
fn build_cors(origins: &[String]) -> CorsLayer {
  let cors = CorsLayer::new()
    .allow_methods([
      Method::GET,
      Method::POST,
      Method::PUT,
      Method::DELETE,
      Method::OPTIONS,
    ])
    // The browser PWA is cross-origin and sends the username + machine-id
    // headers on every request, so they must be on the CORS allow-list or the
    // preflight blocks the call.
    .allow_headers([
      header::AUTHORIZATION,
      header::CONTENT_TYPE,
      HeaderName::from_static(USER_HEADER),
      HeaderName::from_static(MACHINE_ID_HEADER),
    ]);
  if origins.iter().any(|o| o == "*") {
    cors.allow_origin(AllowOrigin::any())
  } else {
    let allowed: Vec<HeaderValue> =
      origins.iter().filter_map(|o| o.parse().ok()).collect();
    cors.allow_origin(AllowOrigin::list(allowed))
  }
}

/// Serve an already-assembled router on a bound listener until shutdown. This
/// is how a downstream crate serves its *composed* router —
/// `layers(routes(state).merge(own_router), &config)` — so the whole thing runs
/// in one process.
pub async fn serve_on(
  listener: tokio::net::TcpListener,
  router: Router,
) -> std::io::Result<()> {
  axum::serve(listener, router).await
}

/// Serve the standalone self-host [`app`] on an already-bound listener until
/// shutdown. Used by `main` and by integration tests that bind an ephemeral
/// port.
pub async fn serve(
  listener: tokio::net::TcpListener,
  state: AppState,
) -> std::io::Result<()> {
  serve_on(listener, app(state)).await
}

async fn health() -> Json<serde_json::Value> {
  Json(json!({ "status": "ok" }))
}
