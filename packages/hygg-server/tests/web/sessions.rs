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
use hygg_server::entity::sessions;
use sea_orm::sea_query::Expr;
use sea_orm::*;

#[tokio::test]
async fn web_session_survives_state_rebuild_and_tracks_metadata() {
  let (dir, state) = migrated_state().await;
  let tenant_id = seed_admin_and_user(&state).await;

  let login = post_form_with_headers(
    state.clone(),
    "/login",
    "email=reader%40example.test&password=readerpass123",
    None,
    &[
      (header::USER_AGENT.as_str(), "hygg-web-test/1.0"),
      ("x-forwarded-for", "203.0.113.7"),
    ],
  )
  .await;
  assert_eq!(login.status(), StatusCode::SEE_OTHER);
  let cookie = session_cookie(&login);

  let reader = repo::users::find_by_email(
    &state.db.conn,
    &tenant_id,
    "reader@example.test",
  )
  .await
  .unwrap()
  .unwrap();
  let sessions =
    repo::sessions::list_for_user(&state.db.conn, &tenant_id, &reader.id)
      .await
      .unwrap();
  assert_eq!(sessions.len(), 1);
  assert_eq!(sessions[0].ip.as_deref(), Some("203.0.113.7"));
  assert_eq!(sessions[0].user_agent.as_deref(), Some("hygg-web-test/1.0"));
  assert!(sessions[0].last_used_at.is_some());

  let session_id = session_id_from_cookie(&cookie);
  let old_last_used = chrono::Utc::now().timestamp_millis() - 60_000;
  let near_expiry = chrono::Utc::now().timestamp_millis() + 1_000;
  sessions::Entity::update_many()
    .col_expr(sessions::Column::ExpiresAt, Expr::value(near_expiry))
    .col_expr(sessions::Column::LastUsedAt, Expr::value(old_last_used))
    .filter(sessions::Column::Id.eq(session_id.clone()))
    .exec(&state.db.conn)
    .await
    .unwrap();
  let account = get(state.clone(), "/account", Some(&cookie)).await;
  assert_eq!(account.status(), StatusCode::OK);
  let refreshed_cookie = account
    .headers()
    .get(header::SET_COOKIE)
    .and_then(|value| value.to_str().ok())
    .unwrap_or_default()
    .to_string();
  assert!(refreshed_cookie.contains("Max-Age=86400"));
  let renewed =
    repo::sessions::list_for_user(&state.db.conn, &tenant_id, &reader.id)
      .await
      .unwrap();
  let renewed_session =
    renewed.iter().find(|session| session.id == session_id).unwrap();
  assert!(renewed_session.expires_at > near_expiry + 23 * 60 * 60 * 1000);
  assert!(renewed_session.last_used_at.unwrap() > old_last_used);

  let url = format!("sqlite://{}", dir.path().join("web.db").display());
  let db = Db::connect(&url).await.unwrap();
  let restarted_state = AppState::new(db, Config::from_env());
  let account = get(restarted_state, "/account", Some(&cookie)).await;
  assert_eq!(account.status(), StatusCode::OK);
  assert!(body_text(account).await.contains("reader@example.test"));
}

#[tokio::test]
async fn user_and_admin_can_revoke_browser_sessions() {
  let (_dir, state) = migrated_state().await;
  let tenant_id = seed_admin_and_user(&state).await;
  let reader = repo::users::find_by_email(
    &state.db.conn,
    &tenant_id,
    "reader@example.test",
  )
  .await
  .unwrap()
  .unwrap();

  let reader_login_1 = post_form(
    state.clone(),
    "/login",
    "email=reader%40example.test&password=readerpass123",
    None,
  )
  .await;
  let reader_cookie_1 = session_cookie(&reader_login_1);
  let reader_login_2 = post_form(
    state.clone(),
    "/login",
    "email=reader%40example.test&password=readerpass123",
    None,
  )
  .await;
  let reader_cookie_2 = session_cookie(&reader_login_2);
  let reader_session_2 = session_id_from_cookie(&reader_cookie_2);
  let reader_token =
    register_device_for(state.clone(), "reader@example.test", "readerpass123")
      .await;
  let reader_token_prefix = reader_token.split('.').next().unwrap().to_string();
  let me =
    get_api(state.clone(), "/api/v1/me", &reader_token, "reader@example.test")
      .await;
  assert_eq!(me.status(), StatusCode::OK);

  let old_sessions_page =
    get(state.clone(), "/account/sessions", Some(&reader_cookie_1)).await;
  assert_eq!(old_sessions_page.status(), StatusCode::SEE_OTHER);
  assert_eq!(location(&old_sessions_page), Some("/account"));

  let account_page =
    get(state.clone(), "/account", Some(&reader_cookie_1)).await;
  assert_eq!(account_page.status(), StatusCode::OK);
  let sessions_html = body_text(account_page).await;
  assert!(sessions_html.contains("Revoke all sessions"));
  // Sessions and the browser-session table are merged into one card; the
  // wording survives in the merged description.
  assert!(sessions_html.contains("Browser sessions"));
  assert!(sessions_html.contains("Last activity"));
  assert!(sessions_html.contains(&reader_session_2[..12]));
  // The account page no longer surfaces device API tokens.
  assert!(!sessions_html.contains("Device API tokens"));
  assert!(!sessions_html.contains("tokens do not expire automatically"));
  assert!(!sessions_html.contains(&reader_token_prefix));
  // Passkeys are a single merged card: the add form plus the list.
  assert!(sessions_html.contains("Add passkey"));
  // The account summary carries no label row of its own; one only appears
  // when an extension injects it.
  assert!(!sessions_html.contains("<span>Plan</span>"));
  let csrf = csrf_token(&sessions_html);

  let revoked = post_form(
    state.clone(),
    &format!("/account/sessions/{reader_session_2}/revoke"),
    &format!("csrf={csrf}"),
    Some(&reader_cookie_1),
  )
  .await;
  assert_eq!(revoked.status(), StatusCode::SEE_OTHER);
  assert_eq!(
    get(state.clone(), "/account", Some(&reader_cookie_2)).await.status(),
    StatusCode::SEE_OTHER
  );
  assert_eq!(
    get(state.clone(), "/account", Some(&reader_cookie_1)).await.status(),
    StatusCode::OK
  );

  let admin_login = post_form(
    state.clone(),
    "/login",
    "email=admin%40example.test&password=adminpass123",
    None,
  )
  .await;
  let admin_cookie = session_cookie(&admin_login);
  let admin_users =
    get(state.clone(), "/app/admin/users", Some(&admin_cookie)).await;
  let admin_users_html = body_text(admin_users).await;
  assert!(
    admin_users_html
      .contains(&format!("/app/admin/users/{}/sessions", reader.id))
  );

  let admin_sessions = get(
    state.clone(),
    &format!("/app/admin/users/{}/sessions", reader.id),
    Some(&admin_cookie),
  )
  .await;
  assert_eq!(admin_sessions.status(), StatusCode::OK);
  let admin_sessions_html = body_text(admin_sessions).await;
  assert!(admin_sessions_html.contains("Device API tokens"));
  assert!(admin_sessions_html.contains(&reader_token_prefix));
  let admin_csrf = csrf_token(&admin_sessions_html);

  let revoked_all = post_form(
    state.clone(),
    &format!("/app/admin/users/{}/sessions/revoke-all", reader.id),
    &format!("csrf={admin_csrf}"),
    Some(&admin_cookie),
  )
  .await;
  assert_eq!(revoked_all.status(), StatusCode::SEE_OTHER);
  assert_eq!(
    get(state.clone(), "/account", Some(&reader_cookie_1)).await.status(),
    StatusCode::SEE_OTHER
  );
  assert_eq!(
    get(state, "/app/admin/users", Some(&admin_cookie)).await.status(),
    StatusCode::OK
  );
}
