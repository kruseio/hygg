use axum::http::{StatusCode, header};
use hygg_server::repo;

use crate::helpers::*;
use hygg_server::entity::applied_ops;
use sea_orm::*;

// Any plan backoffice is an extension's own route; this suite covers the core
// admin dashboard.

#[tokio::test]
async fn admin_dashboard_shows_metrics_and_kpis() {
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
  repo::books::upsert(
    &state.db.conn,
    &tenant_id,
    &reader.id,
    &repo::books::BookInput {
      content_hash: "dashboard-book",
      title: "Dashboard Book",
      author: "",
      format: "txt",
      size_bytes: 128,
    },
  )
  .await
  .unwrap();
  repo::progress::upsert(
    &state.db.conn,
    &tenant_id,
    &reader.id,
    &hygg_server::repo::progress::ProgressInput {
      book_id: "dashboard-book".to_string(),
      device_id: None,
      offset_line: 10,
      total_lines: 100,
      percentage: 10.0,
      viewport_offset: None,
      cursor_y: None,
      page: None,
      line_in_page: None,
      word_offset: None,
      op_id: "dashboard-progress".to_string(),
      updated_at: hygg_server::util::now_millis(),
    },
  )
  .await
  .unwrap();
  applied_ops::ActiveModel {
    tenant_id: Set(tenant_id.clone()),
    op_id: Set("dashboard-op".to_owned()),
    applied_at: Set(hygg_server::util::now_millis()),
  }
  .insert(&state.db.conn)
  .await
  .unwrap();

  let login = post_form_with_headers(
    state.clone(),
    "/login",
    "email=admin%40example.test&password=adminpass123",
    None,
    &[(
      header::USER_AGENT.as_str(),
      "Mozilla/5.0 (Windows NT 10.0; Win64; x64)",
    )],
  )
  .await;
  let cookie = session_cookie(&login);
  let dashboard =
    get(state.clone(), "/app/admin/dashboard", Some(&cookie)).await;
  assert_eq!(dashboard.status(), StatusCode::OK);
  let html = body_text(dashboard).await;
  assert!(html.contains("Dashboard"));
  assert!(html.contains("Last 30 days"));
  assert!(html.contains("Users"));
  assert!(html.contains("Admins"));
  assert!(html.contains("Sync ops"));
  assert!(html.contains("Access Mix"));
  assert!(html.contains("Client Operating Systems"));
  assert!(html.contains("Windows"));
  assert!(html.contains("Activation Funnel"));
  assert!(html.contains("Top Interactions"));
  assert!(html.contains("Progress"));
  assert!(html.contains("Resource Metrics"));
  // Any extra dashboard panels are injected by the web extension, not the
  // core, so nothing deployment-specific appears here.
  assert!(!html.contains("Paying users"));
  assert!(!html.contains("Tier Distribution"));
  // Storage is broken out into document bytes and metadata bytes.
  assert!(html.contains("documents · "));
  assert!(html.contains(" metadata"));
  // Live process/host resource metrics are surfaced.
  assert!(html.contains("Server Resources"));
  assert!(html.contains("Process CPU"));
  assert!(html.contains("Process memory"));
  assert!(html.contains("Network"));
}
