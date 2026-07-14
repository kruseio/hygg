use axum::http::StatusCode;
use hygg_server::repo;

use crate::helpers::*;

/// The admin org wizard creates the org with the chosen owner, and the detail
/// page manages members, default permission, and the last-owner guard. (Plan
/// provisioning is injected by an extension and covered wherever it lives.)
#[tokio::test]
async fn admin_org_wizard_creates_org_and_manages_members() {
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
  let member_id = repo::users::insert(
    &state.db.conn,
    &tenant_id,
    "member@example.test",
    "Member",
    None,
    "user",
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

  let page =
    get(state.clone(), "/app/admin/organizations", Some(&cookie)).await;
  assert_eq!(page.status(), StatusCode::OK);
  let csrf = csrf_token(&body_text(page).await);

  // Create the org with the reader as first owner.
  let created = post_form(
    state.clone(),
    "/app/admin/organizations",
    &format!("csrf={csrf}&name=Acme&owner_user_id={}", reader.id),
    Some(&cookie),
  )
  .await;
  assert_eq!(created.status(), StatusCode::SEE_OTHER);
  let org_path = location(&created).unwrap().to_string();
  let org_id =
    org_path.strip_prefix("/app/admin/organizations/").unwrap().to_string();

  let orgs = repo::organizations::list_for_tenant(&state.db.conn, &tenant_id)
    .await
    .unwrap();
  let acme = orgs.iter().find(|o| o.name == "Acme").unwrap();
  assert_eq!(acme.member_count, 1, "owner is auto-added");
  assert_eq!(acme.default_access, "read_write");

  // Owner is the reader.
  let owner_role = repo::organizations::user_role(
    &state.db.conn,
    &tenant_id,
    &org_id,
    &reader.id,
  )
  .await
  .unwrap();
  assert_eq!(owner_role.as_deref(), Some("owner"));

  // Add a second member.
  let added = post_form(
    state.clone(),
    &format!("/app/admin/organizations/{org_id}/members"),
    &format!("csrf={csrf}&email=member@example.test&role=member"),
    Some(&cookie),
  )
  .await;
  assert_eq!(added.status(), StatusCode::SEE_OTHER);
  assert_eq!(
    repo::organizations::count_members(&state.db.conn, &tenant_id, &org_id)
      .await
      .unwrap(),
    2
  );

  // Change the org default permission to read-only.
  let settings = post_form(
    state.clone(),
    &org_path,
    &format!("csrf={csrf}&name=Acme&default_access=read"),
    Some(&cookie),
  )
  .await;
  assert_eq!(settings.status(), StatusCode::SEE_OTHER);
  let org =
    repo::organizations::find_by_id(&state.db.conn, &tenant_id, &org_id)
      .await
      .unwrap()
      .unwrap();
  assert_eq!(org.default_access, "read");

  // The last owner cannot be removed.
  let blocked = post_form(
    state.clone(),
    &format!("/app/admin/organizations/{org_id}/members/{}/remove", reader.id),
    &format!("csrf={csrf}"),
    Some(&cookie),
  )
  .await;
  assert_eq!(blocked.status(), StatusCode::FORBIDDEN);
  assert!(
    repo::organizations::user_can_access(
      &state.db.conn,
      &tenant_id,
      &org_id,
      &reader.id
    )
    .await
    .unwrap()
  );

  // Promote the member to owner, then the original owner can be removed.
  let promoted = post_form(
    state.clone(),
    &format!("/app/admin/organizations/{org_id}/members/{member_id}/role"),
    &format!("csrf={csrf}&role=owner"),
    Some(&cookie),
  )
  .await;
  assert_eq!(promoted.status(), StatusCode::SEE_OTHER);
  let removed = post_form(
    state.clone(),
    &format!("/app/admin/organizations/{org_id}/members/{}/remove", reader.id),
    &format!("csrf={csrf}"),
    Some(&cookie),
  )
  .await;
  assert_eq!(removed.status(), StatusCode::SEE_OTHER);
  assert_eq!(
    repo::organizations::count_owners(&state.db.conn, &tenant_id, &org_id)
      .await
      .unwrap(),
    1
  );
}
