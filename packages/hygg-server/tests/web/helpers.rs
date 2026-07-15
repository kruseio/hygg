use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use hygg_server::auth::password::hash_password;
use hygg_server::bootstrap::ensure_default_tenant;
use hygg_server::config::Config;
use hygg_server::db::Db;
use hygg_server::state::AppState;
use hygg_server::{app, repo};
use serde_json::json;
use tower::ServiceExt;

pub(crate) async fn migrated_state() -> (tempfile::TempDir, AppState) {
  let dir = tempfile::tempdir().unwrap();
  let url = format!("sqlite://{}", dir.path().join("web.db").display());
  let db = Db::connect(&url).await.unwrap();
  db.migrate().await.unwrap();
  (dir, AppState::new(db, Config::from_env()))
}
pub(crate) async fn seed_admin_and_user(state: &AppState) -> String {
  let tenant_id = ensure_default_tenant(state).await.unwrap();
  let admin_hash = hash_password("adminpass123").unwrap();
  repo::users::insert(
    &state.db.conn,
    &tenant_id,
    "admin@example.test",
    "Admin",
    Some(&admin_hash),
    "admin",
  )
  .await
  .unwrap();
  let reader_hash = hash_password("readerpass123").unwrap();
  repo::users::insert(
    &state.db.conn,
    &tenant_id,
    "reader@example.test",
    "Reader",
    Some(&reader_hash),
    "user",
  )
  .await
  .unwrap();
  tenant_id
}

pub(crate) async fn register_device_for(
  state: AppState,
  email: &str,
  password: &str,
) -> String {
  let response = post_json(
    state,
    "/api/v1/devices/register",
    json!({ "email": email, "password": password, "device_name": "test" }),
    None,
    None,
  )
  .await;
  assert_eq!(response.status(), StatusCode::OK);
  let body: serde_json::Value =
    serde_json::from_str(&body_text(response).await).unwrap();
  body["token"].as_str().unwrap().to_string()
}

/// A fixed machine id for web-suite API calls (bearer auth now also requires
/// the owner's username and a machine id; the device binds to it on first use).
pub(crate) const WEB_MACHINE: &str = "web-test-machine";

pub(crate) async fn push_progress(
  state: AppState,
  token: &str,
  email: &str,
  op_id: &str,
  offset: i64,
) {
  let request = Request::builder()
    .method("POST")
    .uri("/api/v1/sync/push")
    .header(header::AUTHORIZATION, format!("Bearer {token}"))
    .header("x-hygg-user", email)
    .header("x-hygg-machine-id", WEB_MACHINE)
    .header(header::CONTENT_TYPE, "application/json")
    .body(Body::from(
      json!({
        "ops": [{
          "op_id": op_id, "kind": "progress", "book_id": "book-shared",
          "updated_at": offset, "data": { "offset": offset, "total_lines": 100 }
        }]
      })
      .to_string(),
    ))
    .unwrap();
  let response = app(state).oneshot(request).await.unwrap();
  assert_eq!(response.status(), StatusCode::OK);
}

pub(crate) async fn pull_progress_offset(
  state: AppState,
  token: &str,
  email: &str,
) -> i64 {
  let response =
    get_api(state, "/api/v1/sync/pull?since=0", token, email).await;
  assert_eq!(response.status(), StatusCode::OK);
  let body: serde_json::Value =
    serde_json::from_str(&body_text(response).await).unwrap();
  body["progress"][0]["offset_line"].as_i64().unwrap()
}

pub(crate) async fn get(
  state: AppState,
  uri: &str,
  cookie: Option<&str>,
) -> axum::response::Response {
  let mut req = Request::builder().uri(uri);
  if let Some(cookie) = cookie {
    req = req.header(header::COOKIE, cookie);
  }
  app(state).oneshot(req.body(Body::empty()).unwrap()).await.unwrap()
}

pub(crate) async fn get_api(
  state: AppState,
  uri: &str,
  token: &str,
  email: &str,
) -> axum::response::Response {
  app(state)
    .oneshot(
      Request::builder()
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header("x-hygg-user", email)
        .header("x-hygg-machine-id", WEB_MACHINE)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap()
}

pub(crate) async fn post_form(
  state: AppState,
  uri: &str,
  body: &str,
  cookie: Option<&str>,
) -> axum::response::Response {
  post_form_with_headers(state, uri, body, cookie, &[]).await
}

pub(crate) async fn post_form_with_headers(
  state: AppState,
  uri: &str,
  body: &str,
  cookie: Option<&str>,
  headers: &[(&str, &str)],
) -> axum::response::Response {
  let mut req = Request::builder()
    .method("POST")
    .uri(uri)
    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded");
  if let Some(cookie) = cookie {
    req = req.header(header::COOKIE, cookie);
  }
  for (name, value) in headers {
    req = req.header(*name, *value);
  }
  app(state)
    .oneshot(req.body(Body::from(body.to_string())).unwrap())
    .await
    .unwrap()
}

pub(crate) async fn post_json(
  state: AppState,
  uri: &str,
  body: serde_json::Value,
  cookie: Option<&str>,
  csrf: Option<&str>,
) -> axum::response::Response {
  let mut req = Request::builder()
    .method("POST")
    .uri(uri)
    .header(header::CONTENT_TYPE, "application/json");
  if let Some(cookie) = cookie {
    req = req.header(header::COOKIE, cookie);
  }
  if let Some(csrf) = csrf {
    req = req.header("x-csrf-token", csrf);
  }
  app(state)
    .oneshot(req.body(Body::from(body.to_string())).unwrap())
    .await
    .unwrap()
}

pub(crate) async fn body_text(resp: axum::response::Response) -> String {
  let bytes = resp.into_body().collect().await.unwrap().to_bytes();
  String::from_utf8(bytes.to_vec()).unwrap()
}

pub(crate) fn session_cookie(resp: &axum::response::Response) -> String {
  resp
    .headers()
    .get(header::SET_COOKIE)
    .and_then(|v| v.to_str().ok())
    .unwrap()
    .split(';')
    .next()
    .unwrap()
    .to_string()
}

pub(crate) fn location(resp: &axum::response::Response) -> Option<&str> {
  resp.headers().get(header::LOCATION).and_then(|value| value.to_str().ok())
}

pub(crate) fn session_id_from_cookie(cookie: &str) -> String {
  cookie.strip_prefix("hygg_session=").unwrap().to_string()
}

pub(crate) fn csrf_token(html: &str) -> String {
  html
    .split(r#"name="csrf" value=""#)
    .nth(1)
    .and_then(|s| s.split('"').next())
    .unwrap()
    .to_string()
}

pub(crate) fn secret(html: &str) -> String {
  html
    .split(r#"<pre class="secret">"#)
    .nth(1)
    .and_then(|s| s.split("</pre>").next())
    .unwrap()
    .to_string()
}
