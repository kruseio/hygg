//! The hardened device-auth rules: a bearer token is only accepted together
//! with the owner's username and a machine id, the token binds to the first
//! machine it is seen with (and rejects any other), and failed attempts are
//! rate-limited per IP so the endpoint can't be used to spray credentials.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::response::Response;
use http_body_util::BodyExt;
use hygg_server::auth::password::hash_password;
use hygg_server::bootstrap::ensure_default_tenant;
use hygg_server::config::Config;
use hygg_server::db::Db;
use hygg_server::entity::devices;
use hygg_server::state::AppState;
use hygg_server::{app, repo};
use sea_orm::EntityTrait;
use serde_json::{Value, json};
use tower::ServiceExt;

const EMAIL: &str = "u@x.y";
const PASSWORD: &str = "pw";

/// Migrated DB with one regular user.
async fn setup() -> (tempfile::TempDir, AppState) {
  let dir = tempfile::tempdir().unwrap();
  let url = format!("sqlite://{}", dir.path().join("t.db").display());
  let db = Db::connect(&url).await.unwrap();
  db.migrate().await.unwrap();
  let state = AppState::new(db, Config::from_env());
  let tenant_id = ensure_default_tenant(&state).await.unwrap();
  let hash = hash_password(PASSWORD).unwrap();
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

async fn json_body(resp: Response) -> Value {
  let bytes = resp.into_body().collect().await.unwrap().to_bytes();
  serde_json::from_slice(&bytes).unwrap()
}

/// Register a device (optionally binding a machine id at creation) and return
/// its bearer token.
async fn register(state: &AppState, machine_id: Option<&str>) -> String {
  let mut body = json!({ "email": EMAIL, "password": PASSWORD });
  if let Some(m) = machine_id {
    body["machine_id"] = json!(m);
  }
  let req = Request::builder()
    .method("POST")
    .uri("/api/v1/devices/register")
    .header(header::CONTENT_TYPE, "application/json")
    .body(Body::from(body.to_string()))
    .unwrap();
  let resp = app(state.clone()).oneshot(req).await.unwrap();
  assert_eq!(resp.status(), StatusCode::OK);
  json_body(resp).await["token"].as_str().unwrap().to_string()
}

/// Call `/me` with an explicitly chosen (or omitted) username and machine id.
async fn me(
  state: &AppState,
  token: &str,
  user: Option<&str>,
  machine: Option<&str>,
) -> StatusCode {
  let mut builder = Request::builder()
    .uri("/api/v1/me")
    .header(header::AUTHORIZATION, format!("Bearer {token}"));
  if let Some(user) = user {
    builder = builder.header("x-hygg-user", user);
  }
  if let Some(machine) = machine {
    builder = builder.header("x-hygg-machine-id", machine);
  }
  let req = builder.body(Body::empty()).unwrap();
  app(state.clone()).oneshot(req).await.unwrap().status()
}

#[tokio::test]
async fn token_binds_to_first_machine_and_rejects_others() {
  let (_dir, state) = setup().await;
  let token = register(&state, None).await;

  // First use binds the token to "laptop"; the same machine keeps working.
  assert_eq!(
    me(&state, &token, Some(EMAIL), Some("laptop")).await,
    StatusCode::OK
  );
  assert_eq!(
    me(&state, &token, Some(EMAIL), Some("laptop")).await,
    StatusCode::OK
  );

  // The very same token from a different machine is refused.
  assert_eq!(
    me(&state, &token, Some(EMAIL), Some("phone")).await,
    StatusCode::UNAUTHORIZED
  );

  // The binding is recorded on the device row.
  let bound =
    devices::Entity::find().one(&state.db.conn).await.unwrap().unwrap();
  assert_eq!(bound.machine_id.as_deref(), Some("laptop"));
}

#[tokio::test]
async fn registration_can_bind_the_machine_up_front() {
  let (_dir, state) = setup().await;
  // Registering with a machine id locks the device immediately.
  let token = register(&state, Some("laptop")).await;
  assert_eq!(
    me(&state, &token, Some(EMAIL), Some("laptop")).await,
    StatusCode::OK
  );
  assert_eq!(
    me(&state, &token, Some(EMAIL), Some("phone")).await,
    StatusCode::UNAUTHORIZED
  );
}

#[tokio::test]
async fn missing_machine_id_is_rejected() {
  let (_dir, state) = setup().await;
  let token = register(&state, None).await;
  assert_eq!(
    me(&state, &token, Some(EMAIL), None).await,
    StatusCode::UNAUTHORIZED
  );
}

#[tokio::test]
async fn username_is_required_and_must_match_owner() {
  let (_dir, state) = setup().await;
  let token = register(&state, None).await;

  // No username header: rejected.
  assert_eq!(
    me(&state, &token, None, Some("laptop")).await,
    StatusCode::UNAUTHORIZED
  );
  // Wrong username: rejected.
  assert_eq!(
    me(&state, &token, Some("someone@else.test"), Some("laptop")).await,
    StatusCode::UNAUTHORIZED
  );
  // Correct username (case-insensitive): accepted.
  assert_eq!(
    me(&state, &token, Some("U@X.Y"), Some("laptop")).await,
    StatusCode::OK
  );
}

#[tokio::test]
async fn repeated_failures_are_rate_limited() {
  let (_dir, state) = setup().await;
  let good = register(&state, Some("laptop")).await;

  // Ten bad-token attempts are each unauthorized (and each records a failure).
  for _ in 0..10 {
    assert_eq!(
      me(&state, "bogus.prefix", Some(EMAIL), Some("laptop")).await,
      StatusCode::UNAUTHORIZED
    );
  }

  // The next attempt is blocked with 429 — even a fully valid credential,
  // proving the limiter fronts the auth check.
  assert_eq!(
    me(&state, &good, Some(EMAIL), Some("laptop")).await,
    StatusCode::TOO_MANY_REQUESTS
  );
}
