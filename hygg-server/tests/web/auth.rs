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
async fn signup_creates_regular_user_and_session() {
  let (_dir, state) = migrated_state().await;
  let resp = post_form(
    state.clone(),
    "/signup",
    "email=new%40example.test&display_name=New&password=password123",
    None,
  )
  .await;
  assert_eq!(resp.status(), StatusCode::SEE_OTHER);
  let cookie = session_cookie(&resp);

  let tenant = repo::tenants::find_id_by_slug(&state.db.conn, "default")
    .await
    .unwrap()
    .unwrap();
  let user =
    repo::users::find_by_email(&state.db.conn, &tenant, "new@example.test")
      .await
      .unwrap()
      .unwrap();
  // The open core models only admin vs. user; signups are plain users.
  assert_eq!(user.role, "user");

  let account = get(state, "/account", Some(&cookie)).await;
  assert_eq!(account.status(), StatusCode::OK);
  let html = body_text(account).await;
  assert!(html.contains("new@example.test"));
  // The account page shows no label of its own; one only appears if an
  // override injects a row.
  assert!(!html.contains("nonpaying"));
  assert!(!html.contains("Tier"));
}

#[tokio::test]
async fn weak_signup_password_keeps_values_and_shows_toast() {
  let (_dir, state) = migrated_state().await;
  let resp = post_form(
    state,
    "/signup",
    "email=weak%40example.test&display_name=Weak+Reader&password=short",
    None,
  )
  .await;
  assert_eq!(resp.status(), StatusCode::OK);
  let html = body_text(resp).await;
  assert!(html.contains("Password must be at least 8 characters."));
  assert!(html.contains(r#"class="toast toast-error""#));
  assert!(html.contains(r#"data-password-policy="signup""#));
  assert!(html.contains(r#"value="weak@example.test""#));
  assert!(html.contains(r#"value="Weak Reader""#));
  assert!(html.contains(r#"value="short""#));
}

#[tokio::test]
async fn recovery_token_logs_in_once_without_reenabling_password_auth() {
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
  repo::users::set_password_enabled(
    &state.db.conn,
    &tenant_id,
    &user.id,
    false,
  )
  .await
  .unwrap();

  let login = post_form(
    state.clone(),
    "/login",
    "email=admin%40example.test&password=adminpass123",
    None,
  )
  .await;
  let cookie = session_cookie(&login);
  let users_page = get(state.clone(), "/app/admin/users", Some(&cookie)).await;
  let users_html = body_text(users_page).await;
  assert!(users_html.contains("Recovery token"));
  let csrf = csrf_token(&users_html);
  let recovery = post_form(
    state.clone(),
    &format!("/app/admin/users/{}/recovery", user.id),
    &format!("csrf={csrf}"),
    Some(&cookie),
  )
  .await;
  let recovery_html = body_text(recovery).await;
  assert!(recovery_html.contains("Recovery token"));
  assert!(recovery_html.contains("expires in 30 minutes"));
  assert!(recovery_html.contains("does not re-enable the user"));
  let code = secret(&recovery_html);

  let regular = post_form(
    state.clone(),
    "/login",
    "email=reader%40example.test&password=readerpass123",
    None,
  )
  .await;
  assert_eq!(regular.status(), StatusCode::OK);
  assert!(body_text(regular).await.contains("Invalid credentials"));

  let login_step =
    post_form(state.clone(), "/login", "email=reader%40example.test", None)
      .await;
  let login_step_html = body_text(login_step).await;
  assert!(login_step_html.contains(r#"name="recovery_token""#));
  assert!(!login_step_html.contains(r#"name="password""#));

  let body = format!("email=reader%40example.test&recovery_token={code}");
  let recovered = post_form(state.clone(), "/login", &body, None).await;
  assert_eq!(recovered.status(), StatusCode::SEE_OTHER);
  assert!(session_cookie(&recovered).starts_with("hygg_session="));

  let user = repo::users::find_by_id(&state.db.conn, &tenant_id, &user.id)
    .await
    .unwrap()
    .unwrap();
  assert_eq!(user.password_enabled, 0);

  let reused = post_form(state, "/login", &body, None).await;
  assert_eq!(reused.status(), StatusCode::OK);
  assert!(body_text(reused).await.contains("Invalid credentials"));
}

#[tokio::test]
async fn login_identifier_step_is_rate_limited() {
  let (_dir, state) = migrated_state().await;
  seed_admin_and_user(&state).await;

  for _ in 0..8 {
    let resp = post_form_with_headers(
      state.clone(),
      "/login",
      "email=missing%40example.test",
      None,
      &[("x-forwarded-for", "203.0.113.10")],
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
  }

  let limited = post_form_with_headers(
    state,
    "/login",
    "email=missing%40example.test",
    None,
    &[("x-forwarded-for", "203.0.113.10")],
  )
  .await;
  assert_eq!(limited.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn login_step_hides_password_when_password_auth_is_disabled() {
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
  repo::users::set_password_enabled(
    &state.db.conn,
    &tenant_id,
    &user.id,
    false,
  )
  .await
  .unwrap();

  let login_step =
    post_form(state, "/login", "email=reader%40example.test", None).await;
  assert_eq!(login_step.status(), StatusCode::OK);
  let html = body_text(login_step).await;
  assert!(html.contains("Signing in as <strong>reader@example.test</strong>"));
  assert!(html.contains("Password auth is disabled"));
  assert!(html.contains(r#"name="recovery_token""#));
  assert!(!html.contains(r#"name="password""#));
}
