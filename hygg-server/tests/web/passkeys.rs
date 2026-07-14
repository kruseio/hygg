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
async fn password_auth_cannot_be_disabled_without_valid_passkey() {
  let (_dir, state) = migrated_state().await;
  let tenant_id = seed_admin_and_user(&state).await;
  let user = repo::users::find_by_email(
    &state.db.conn,
    &tenant_id,
    "reader@example.test",
  )
  .await
  .unwrap()
  .unwrap();

  let login = post_form(
    state.clone(),
    "/login",
    "email=reader%40example.test&password=readerpass123",
    None,
  )
  .await;
  let cookie = session_cookie(&login);
  let account = get(state.clone(), "/account", Some(&cookie)).await;
  let account_html = body_text(account).await;
  assert!(account_html.contains(r#"name="password_enabled" value="disabled""#));
  assert!(account_html.contains(
    "value=\"disabled\" disabled title=\"A valid passkey is required\""
  ));
  let csrf = csrf_token(&account_html);

  let disabled = post_form(
    state.clone(),
    "/account/password",
    &format!("csrf={csrf}&action=password_status&password_enabled=disabled"),
    Some(&cookie),
  )
  .await;
  assert_eq!(disabled.status(), StatusCode::FORBIDDEN);

  let user = repo::users::find_by_id(&state.db.conn, &tenant_id, &user.id)
    .await
    .unwrap()
    .unwrap();
  assert_eq!(user.password_enabled, 1);
}

#[tokio::test]
async fn passkey_pages_expose_registration_and_login_ceremonies() {
  let (_dir, state) = migrated_state().await;
  seed_admin_and_user(&state).await;

  let login_page = get(state.clone(), "/login", None).await;
  let login_html = body_text(login_page).await;
  assert!(login_html.contains(r#"data-login-step="identifier""#));
  assert!(login_html.contains(r#"autocomplete="username webauthn""#));
  assert!(login_html.contains(r#"event.key !== "Enter""#));
  assert!(login_html.contains("requestSubmit"));
  assert!(!login_html.contains(r#"type="password""#));

  let login_step =
    post_form(state.clone(), "/login", "email=reader%40example.test", None)
      .await;
  assert_eq!(login_step.status(), StatusCode::OK);
  let login_step_html = body_text(login_step).await;
  assert!(
    login_step_html
      .contains("Signing in as <strong>reader@example.test</strong>")
  );
  assert!(login_step_html.contains("Use password"));
  assert!(login_step_html.contains(r#"type="password""#));
  assert!(!login_step_html.contains(r#"id="passkey-login""#));

  let login = post_form(
    state.clone(),
    "/login",
    "email=reader%40example.test&password=readerpass123",
    None,
  )
  .await;
  let cookie = session_cookie(&login);

  let account_page = get(state.clone(), "/account", Some(&cookie)).await;
  assert_eq!(account_page.status(), StatusCode::OK);
  let passkeys_html = body_text(account_page).await;
  assert!(passkeys_html.contains("class=\"sidenav\""));
  assert!(passkeys_html.contains("class=\"nav-group\""));
  assert!(passkeys_html.contains("class=\"sidenav-toggle-button\""));
  assert!(passkeys_html.contains("class=\"account-menu\""));
  assert!(passkeys_html.contains(r#"href="/account" role="menuitem">"#));
  assert!(
    !passkeys_html.contains(r#"href="/account/sessions" role="menuitem">"#)
  );
  assert!(passkeys_html.contains("<span>Settings</span>"));
  assert!(passkeys_html.contains("lucide lucide-circle-user-round"));
  assert!(passkeys_html.contains("lucide lucide-settings"));
  assert!(passkeys_html.contains("lucide lucide-chevron-down"));
  assert!(!passkeys_html.contains("avatar-img"));
  assert!(!passkeys_html.contains("class=\"nav-links\""));
  assert!(!passkeys_html.contains(r#"href="/account/passkeys""#));
  assert!(passkeys_html.contains("id=\"add-passkey\""));
  assert!(passkeys_html.contains("/webauthn/register/start"));
  let csrf = csrf_token(&passkeys_html);

  let old_passkeys_url =
    get(state.clone(), "/account/passkeys", Some(&cookie)).await;
  assert_eq!(old_passkeys_url.status(), StatusCode::SEE_OTHER);
  assert_eq!(
    old_passkeys_url
      .headers()
      .get(header::LOCATION)
      .and_then(|value| value.to_str().ok()),
    Some("/account")
  );

  let forbidden = post_json(
    state.clone(),
    "/webauthn/register/start",
    json!({ "label": "laptop" }),
    Some(&cookie),
    None,
  )
  .await;
  assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);

  let started = post_json(
    state.clone(),
    "/webauthn/register/start",
    json!({ "label": "laptop" }),
    Some(&cookie),
    Some(&csrf),
  )
  .await;
  assert_eq!(started.status(), StatusCode::OK);
  let body: serde_json::Value =
    serde_json::from_str(&body_text(started).await).unwrap();
  assert!(body["requestId"].as_str().is_some());
  assert!(body["options"]["publicKey"]["challenge"].as_str().is_some());

  let no_passkey_login = post_json(
    state,
    "/webauthn/auth/start",
    json!({ "email": "reader@example.test" }),
    None,
    None,
  )
  .await;
  assert_eq!(no_passkey_login.status(), StatusCode::UNAUTHORIZED);
}
