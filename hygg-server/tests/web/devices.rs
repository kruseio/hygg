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

use crate::helpers::*;

#[tokio::test]
async fn admin_can_create_device_with_document_permission_override() {
  let (_dir, state) = migrated_state().await;
  let tenant_id = seed_admin_and_user(&state).await;

  let login = post_form(
    state.clone(),
    "/login",
    "email=admin%40example.test&password=adminpass123",
    None,
  )
  .await;
  let cookie = session_cookie(&login);
  let admin_devices =
    get(state.clone(), "/app/admin/devices", Some(&cookie)).await;
  let csrf = csrf_token(&body_text(admin_devices).await);

  let user = repo::users::find_by_email(
    &state.db.conn,
    &tenant_id,
    "reader@example.test",
  )
  .await
  .unwrap()
  .unwrap();
  repo::books::upsert(
    &state.db.conn,
    &tenant_id,
    &user.id,
    &repo::books::BookInput {
      content_hash: "book-allowed",
      title: "Allowed book",
      author: "",
      format: "txt",
      size_bytes: 42,
    },
  )
  .await
  .unwrap();
  let body = format!(
    "csrf={csrf}&user_id={}&name=tablet&platform=ios&default_access=none",
    user.id
  );
  let token_page =
    post_form(state.clone(), "/app/admin/devices", &body, Some(&cookie)).await;
  assert_eq!(token_page.status(), StatusCode::OK);
  let token_html = body_text(token_page).await;
  assert!(token_html.contains(r#"data-copy-secret"#));
  assert!(token_html.contains(">Copy</span>"));
  assert!(token_html.contains("Connect the CLI"));
  assert!(token_html.contains(":connect http://localhost:3032"));
  // A back link to the admin devices list, and no manual `:sync` step (sync
  // runs automatically after authentication).
  assert!(
    token_html.contains(r#"<a class="back-link" href="/app/admin/devices">"#),
    "{token_html}"
  );
  // (`:sync` as a CLI command line — not the CSS `animation:sync-dash`.)
  assert!(!token_html.contains("\n:sync"), "{token_html}");
  let token = secret(&token_html);
  // The auth line is prefilled with the device owner's email and the token,
  // ready to copy — no placeholders.
  assert!(!token_html.contains("&lt;your-username&gt;"), "{token_html}");
  assert!(
    token_html.contains(&format!(":auth reader@example.test {token}")),
    "{token_html}"
  );

  let me =
    get_api(state.clone(), "/api/v1/me", &token, "reader@example.test").await;
  assert_eq!(me.status(), StatusCode::OK);
  let me_json: serde_json::Value =
    serde_json::from_str(&body_text(me).await).unwrap();
  assert_eq!(me_json["default_access"], "none");
  assert_eq!(me_json["read_only"], true);
  let device_id = me_json["device_id"].as_str().unwrap();

  let permissions_page = get(
    state.clone(),
    &format!("/app/admin/devices/{device_id}/permissions"),
    Some(&cookie),
  )
  .await;
  assert_eq!(permissions_page.status(), StatusCode::OK);
  let permissions_html = body_text(permissions_page).await;
  assert!(permissions_html.contains("Allowed book"));
  assert!(permissions_html.contains("book_access:book-allowed"));
  let csrf = csrf_token(&permissions_html);
  let permissions_body = format!(
    "csrf={csrf}&default_access=none&book_access:book-allowed=read_write"
  );
  let saved = post_form(
    state.clone(),
    &format!("/app/admin/devices/{device_id}/permissions"),
    &permissions_body,
    Some(&cookie),
  )
  .await;
  assert_eq!(saved.status(), StatusCode::SEE_OTHER);

  let push = Request::builder()
    .method("POST")
    .uri("/api/v1/sync/push")
    .header(header::AUTHORIZATION, format!("Bearer {token}"))
    .header("x-hygg-user", "reader@example.test")
    .header("x-hygg-machine-id", WEB_MACHINE)
    .header(header::CONTENT_TYPE, "application/json")
    .body(Body::from(
      json!({
        "ops": [{
          "op_id": "op1", "kind": "progress", "book_id": "book-allowed",
          "updated_at": 1, "data": { "offset": 1, "total_lines": 2 }
        }, {
          "op_id": "op2", "kind": "progress", "book_id": "book-denied",
          "updated_at": 1, "data": { "offset": 1, "total_lines": 2 }
        }]
      })
      .to_string(),
    ))
    .unwrap();
  let push = app(state.clone()).oneshot(push).await.unwrap();
  assert_eq!(push.status(), StatusCode::OK);
  let body: serde_json::Value =
    serde_json::from_str(&body_text(push).await).unwrap();
  assert_eq!(body["applied"], json!(["op1"]));
  assert_eq!(body["skipped"], json!(["op2"]));
}

#[tokio::test]
async fn devices_page_shows_plain_count_without_plans() {
  let (_dir, state) = migrated_state().await;
  seed_admin_and_user(&state).await;

  let login = post_form(
    state.clone(),
    "/login",
    "email=reader%40example.test&password=readerpass123",
    None,
  )
  .await;
  let cookie = session_cookie(&login);

  // The core has no device caps of its own: a plain count, no quota badge,
  // and the create button stays enabled. Caps are an injected concern.
  let page = get(state.clone(), "/app/devices", Some(&cookie)).await;
  let html = body_text(page).await;
  assert!(
    html.contains(r#"<span class="device-quota">0 devices</span>"#),
    "{html}"
  );
  assert!(!html.contains("0 / "), "no quota badge on self-host: {html}");
  assert!(!html.contains("reached your device limit"), "{html}");
  assert!(
    html.contains("<button type=\"submit\" >Create token</button>"),
    "{html}"
  );
}
