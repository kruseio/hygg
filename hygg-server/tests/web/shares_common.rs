//! Shared helpers for the document-sharing web tests (`shares`,
//! `shares_progress`).

use axum::body::Body;
use axum::http::{StatusCode, header};
use hygg_server::auth::password::hash_password;
use hygg_server::state::AppState;
use hygg_server::{app, repo};
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::helpers::*;

pub(crate) async fn add_user(state: &AppState, tenant: &str, email: &str) {
  let hash = hash_password("friendpass123").unwrap();
  repo::users::insert(
    &state.db.conn,
    tenant,
    email,
    "Friend",
    Some(&hash),
    "user",
  )
  .await
  .unwrap();
}

pub(crate) async fn own_book(
  state: &AppState,
  tenant: &str,
  owner: &str,
  hash: &str,
) {
  repo::books::upsert(
    &state.db.conn,
    tenant,
    owner,
    &repo::books::BookInput {
      content_hash: hash,
      title: hash,
      author: "",
      format: "txt",
      size_bytes: 1,
    },
  )
  .await
  .unwrap();
}

pub(crate) async fn login(
  state: AppState,
  email: &str,
  password: &str,
) -> String {
  let body = format!("email={}&password={password}", email.replace('@', "%40"));
  session_cookie(&post_form(state, "/login", &body, None).await)
}

pub(crate) async fn push_progress_for(
  state: AppState,
  token: &str,
  email: &str,
  machine: &str,
  book: &str,
  offset: i64,
) {
  let req = axum::http::Request::builder()
    .method("POST")
    .uri("/api/v1/sync/push")
    .header(header::AUTHORIZATION, format!("Bearer {token}"))
    .header("x-hygg-user", email)
    .header("x-hygg-machine-id", machine)
    .header(header::CONTENT_TYPE, "application/json")
    .body(Body::from(
      json!({ "ops": [{
        "op_id": format!("{email}-{book}-{offset}"),
        "kind": "progress", "book_id": book, "updated_at": offset,
        "data": { "offset": offset, "total_lines": 100, "percentage": 1.0 }
      }]})
      .to_string(),
    ))
    .unwrap();
  let resp = app(state).oneshot(req).await.unwrap();
  assert_eq!(resp.status(), StatusCode::OK);
}

pub(crate) async fn pull_offset_for(
  state: AppState,
  token: &str,
  email: &str,
  machine: &str,
  book: &str,
) -> Option<i64> {
  let req = axum::http::Request::builder()
    .uri("/api/v1/sync/pull?since=0")
    .header(header::AUTHORIZATION, format!("Bearer {token}"))
    .header("x-hygg-user", email)
    .header("x-hygg-machine-id", machine)
    .body(Body::empty())
    .unwrap();
  let resp = app(state).oneshot(req).await.unwrap();
  let body: Value = serde_json::from_str(&body_text(resp).await).unwrap();
  body["progress"].as_array().unwrap().iter().find_map(|row| {
    (row["book_id"] == book).then(|| row["offset_line"].as_i64().unwrap())
  })
}

pub(crate) async fn books_contains(
  state: AppState,
  token: &str,
  email: &str,
  hash: &str,
) -> bool {
  let resp = get_api(state, "/api/v1/books", token, email).await;
  let body: Value = serde_json::from_str(&body_text(resp).await).unwrap();
  body
    .as_array()
    .unwrap()
    .iter()
    .any(|b| b["content_hash"] == hash || b["book_id"] == hash)
}
