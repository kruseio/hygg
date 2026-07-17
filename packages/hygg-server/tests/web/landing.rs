//! Core chrome/nav behaviour. This exercises only the routes the core serves;
//! an extension's own pages are covered wherever they are defined.

use axum::http::StatusCode;

use crate::helpers::*;

/// On the open self-host server nothing is withheld: an admin gets the admin
/// nav *and* the full workspace (home / devices / organizations). An override
/// can withhold this via the entitlements hook.
#[tokio::test]
async fn admin_sees_admin_and_workspace_nav() {
  let (_dir, state) = migrated_state().await;
  seed_admin_and_user(&state).await;

  let login = post_form(
    state.clone(),
    "/login",
    "email=admin%40example.test&password=adminpass123",
    None,
  )
  .await;
  assert_eq!(login.status(), StatusCode::SEE_OTHER);
  assert_eq!(location(&login), Some("/app/admin/dashboard"));
  let cookie = session_cookie(&login);

  let dashboard =
    get(state.clone(), "/app/admin/dashboard", Some(&cookie)).await;
  assert_eq!(dashboard.status(), StatusCode::OK);
  let html = body_text(dashboard).await;
  assert!(html.contains(r#"href="/app/admin/organizations""#));
  assert!(html.contains(r#"href="/app/organizations""#));
  assert!(html.contains(r#"href="/app/home""#));
  assert!(html.contains(r#"href="/app/devices""#));
  // Any extra backoffice pages are injected, not served by the core.
  assert!(!html.contains(r#"href="/app/admin/injected-one""#));
  assert!(!html.contains(r#"href="/app/admin/injected-two""#));

  let home = get(state.clone(), "/app/home", Some(&cookie)).await;
  assert_eq!(home.status(), StatusCode::OK);

  let devices = get(state, "/app/devices", Some(&cookie)).await;
  assert_eq!(devices.status(), StatusCode::OK);
}

/// A plain signup on self-host lands on the full workspace home (the open core
/// withholds nothing).
#[tokio::test]
async fn regular_user_login_lands_on_home() {
  let (_dir, state) = migrated_state().await;
  seed_admin_and_user(&state).await;

  let login = post_form(
    state.clone(),
    "/login",
    "email=reader%40example.test&password=readerpass123",
    None,
  )
  .await;
  assert_eq!(login.status(), StatusCode::SEE_OTHER);
  assert_eq!(location(&login), Some("/app/home"));
}
