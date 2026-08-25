//! End-to-end encryption enforcement at the API boundary.
//!
//! Once an account turns encryption on, the server must reject any upload that
//! is not a sealed envelope — for both document blobs and note bodies — and
//! must accept sealed ones. These tests drive the real router so the marker,
//! the enforcement, and the wire contract are exercised together.

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
use hygg_shared::crypto;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tower::ServiceExt;

const EMAIL: &str = "e@x.y";
const MACHINE: &str = "enc-machine";
const HASH: &str = "enc-hash-1";

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
    "E",
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
    .body(Body::from(json!({"email": EMAIL, "password": "pw"}).to_string()))
    .unwrap();
  let resp = app(state.clone()).oneshot(req).await.unwrap();
  assert_eq!(resp.status(), StatusCode::OK);
  json_body::<Value>(resp).await["token"].as_str().unwrap().to_string()
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
    .header("x-hygg-machine-id", MACHINE)
}

async fn send(state: &AppState, req: Request<Body>) -> Response {
  app(state.clone()).oneshot(req).await.unwrap()
}

async fn json_body<T: DeserializeOwned>(resp: Response) -> T {
  let bytes = resp.into_body().collect().await.unwrap().to_bytes();
  serde_json::from_slice(&bytes).unwrap()
}

async fn upsert_book(state: &AppState, token: &str) {
  let req = authed("POST", "/api/v1/books", token)
    .header(header::CONTENT_TYPE, "application/json")
    .body(Body::from(
      json!({ "content_hash": HASH, "title": "T", "format": "txt",
              "size_bytes": 3 })
      .to_string(),
    ))
    .unwrap();
  assert_eq!(send(state, req).await.status(), StatusCode::OK);
}

async fn put_blob(state: &AppState, token: &str, bytes: Vec<u8>) -> StatusCode {
  let req = authed("PUT", &format!("/api/v1/books/{HASH}/blob"), token)
    .header(header::CONTENT_TYPE, "application/octet-stream")
    .body(Body::from(bytes))
    .unwrap();
  send(state, req).await.status()
}

/// Enable encryption for the account by publishing a salt + verifier derived
/// from a test key, and return that key so the test can seal payloads with it.
async fn enable_encryption(
  state: &AppState,
  token: &str,
) -> crypto::EncryptionKey {
  use base64::Engine;
  use base64::engine::general_purpose::STANDARD;
  let salt = crypto::random_salt().unwrap();
  let key = crypto::derive_key(b"test passphrase", &salt).unwrap();
  let verifier = crypto::make_verifier(&key).unwrap();
  let req = authed("PUT", "/api/v1/encryption", token)
    .header(header::CONTENT_TYPE, "application/json")
    .body(Body::from(
      json!({
        "kdf": crypto::KDF_ARGON2ID,
        "alg": crypto::ALG_XCHACHA20POLY1305,
        "salt": STANDARD.encode(salt),
        "verifier": STANDARD.encode(verifier),
      })
      .to_string(),
    ))
    .unwrap();
  let resp = send(state, req).await;
  assert_eq!(resp.status(), StatusCode::OK);
  let state_body = json_body::<Value>(resp).await;
  assert_eq!(state_body["enabled"], json!(true));
  key
}

#[tokio::test]
async fn marker_defaults_to_disabled() {
  let (_d, state) = setup().await;
  let token = register(&state).await;
  let req = authed("GET", "/api/v1/encryption", token.as_str())
    .body(Body::empty())
    .unwrap();
  let resp = send(&state, req).await;
  assert_eq!(resp.status(), StatusCode::OK);
  assert_eq!(json_body::<Value>(resp).await["enabled"], json!(false));
}

#[tokio::test]
async fn plaintext_blob_allowed_until_encryption_is_enabled() {
  let (_d, state) = setup().await;
  let token = register(&state).await;
  upsert_book(&state, &token).await;
  // Before enabling, a plaintext upload is fine.
  assert_eq!(put_blob(&state, &token, b"abc".to_vec()).await, StatusCode::OK);
}

#[tokio::test]
async fn plaintext_blob_rejected_once_encryption_is_enabled() {
  let (_d, state) = setup().await;
  let token = register(&state).await;
  upsert_book(&state, &token).await;
  let key = enable_encryption(&state, &token).await;

  // Plaintext is now refused with a conflict.
  assert_eq!(
    put_blob(&state, &token, b"abc".to_vec()).await,
    StatusCode::CONFLICT
  );

  // A sealed envelope under the account key is accepted.
  let sealed = crypto::encrypt(&key, b"abc").unwrap();
  assert_eq!(put_blob(&state, &token, sealed).await, StatusCode::OK);
}

#[tokio::test]
async fn plaintext_note_body_dropped_once_encryption_is_enabled() {
  let (_d, state) = setup().await;
  let token = register(&state).await;
  upsert_book(&state, &token).await;
  let key = enable_encryption(&state, &token).await;

  // A plaintext note body is silently dropped (skipped, not applied).
  let ops = json!({ "ops": [
    { "op_id": "op-plain", "kind": "note", "book_id": HASH, "updated_at": 10,
      "data": { "id": "n1", "body": "readable secret" } }
  ]});
  let resp = send(
    &state,
    authed("POST", "/api/v1/sync/push", &token)
      .header(header::CONTENT_TYPE, "application/json")
      .body(Body::from(ops.to_string()))
      .unwrap(),
  )
  .await;
  let body = json_body::<Value>(resp).await;
  assert_eq!(body["applied"].as_array().unwrap().len(), 0);
  assert_eq!(body["skipped"].as_array().unwrap().len(), 1);

  // A sealed note body is applied.
  let sealed = crypto::encrypt_string(&key, "readable secret").unwrap();
  let ops = json!({ "ops": [
    { "op_id": "op-sealed", "kind": "note", "book_id": HASH, "updated_at": 11,
      "data": { "id": "n1", "body": sealed } }
  ]});
  let resp = send(
    &state,
    authed("POST", "/api/v1/sync/push", &token)
      .header(header::CONTENT_TYPE, "application/json")
      .body(Body::from(ops.to_string()))
      .unwrap(),
  )
  .await;
  let body = json_body::<Value>(resp).await;
  assert_eq!(body["applied"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn disabling_stops_enforcement_and_allows_plaintext_again() {
  let (_d, state) = setup().await;
  let token = register(&state).await;
  upsert_book(&state, &token).await;
  let key = enable_encryption(&state, &token).await;

  // Sealed upload works; plaintext is refused.
  let sealed = crypto::encrypt(&key, b"abc").unwrap();
  assert_eq!(put_blob(&state, &token, sealed).await, StatusCode::OK);
  assert_eq!(
    put_blob(&state, &token, b"abc".to_vec()).await,
    StatusCode::CONFLICT
  );

  // Disable, and the marker reports off.
  let del =
    authed("DELETE", "/api/v1/encryption", &token).body(Body::empty()).unwrap();
  let resp = send(&state, del).await;
  assert_eq!(resp.status(), StatusCode::OK);
  assert_eq!(json_body::<Value>(resp).await["enabled"], json!(false));

  // Plaintext is accepted again (the decrypt-all re-upload path).
  assert_eq!(put_blob(&state, &token, b"abc".to_vec()).await, StatusCode::OK);
}

#[tokio::test]
async fn re_enabling_with_a_different_salt_conflicts() {
  use base64::Engine;
  use base64::engine::general_purpose::STANDARD;
  let (_d, state) = setup().await;
  let token = register(&state).await;
  enable_encryption(&state, &token).await;

  // A second enable under a fresh salt is refused (it would strand documents).
  let other_salt = crypto::random_salt().unwrap();
  let other_key = crypto::derive_key(b"other", &other_salt).unwrap();
  let req = authed("PUT", "/api/v1/encryption", &token)
    .header(header::CONTENT_TYPE, "application/json")
    .body(Body::from(
      json!({
        "kdf": crypto::KDF_ARGON2ID,
        "alg": crypto::ALG_XCHACHA20POLY1305,
        "salt": STANDARD.encode(other_salt),
        "verifier": STANDARD.encode(crypto::make_verifier(&other_key).unwrap()),
      })
      .to_string(),
    ))
    .unwrap();
  assert_eq!(send(&state, req).await.status(), StatusCode::CONFLICT);
}
