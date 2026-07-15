use axum::http::StatusCode;
use hygg_server::auth::AccessLevel;
use hygg_server::repo;

use crate::helpers::*;

/// An org owner drives the management UI end to end — create a directory and a
/// group, add a member to the group, tighten the org default, then grant the
/// group read/write on the directory — and a member's effective access tracks
/// each step through the permission model.
#[tokio::test]
async fn owner_manages_directories_groups_and_permissions() {
  let (_dir, state) = migrated_state().await;
  let tenant = seed_admin_and_user(&state).await;
  let pool = &state.db.conn;
  let owner = repo::users::find_by_email(pool, &tenant, "reader@example.test")
    .await
    .unwrap()
    .unwrap();
  let member = repo::users::insert(
    pool,
    &tenant,
    "member@example.test",
    "M",
    None,
    "user",
  )
  .await
  .unwrap();
  let other =
    repo::users::insert(pool, &tenant, "other@example.test", "O", None, "user")
      .await
      .unwrap();
  let org = repo::organizations::create(pool, &tenant, "Team", &owner.id)
    .await
    .unwrap();
  repo::organizations::add_member(pool, &tenant, &org, &member, "member")
    .await
    .unwrap();
  repo::organizations::add_member(pool, &tenant, &org, &other, "member")
    .await
    .unwrap();
  repo::books::upsert(
    pool,
    &tenant,
    &owner.id,
    &repo::books::BookInput {
      content_hash: "doc1",
      title: "Doc One",
      author: "",
      format: "txt",
      size_bytes: 5,
    },
  )
  .await
  .unwrap();
  repo::books::move_to_organization(
    pool,
    &tenant,
    &owner.id,
    "doc1",
    Some(&org),
  )
  .await
  .unwrap();

  let login = post_form(
    state.clone(),
    "/login",
    "email=reader%40example.test&password=readerpass123",
    None,
  )
  .await;
  let cookie = session_cookie(&login);
  let page =
    get(state.clone(), &format!("/app/organizations/{org}"), Some(&cookie))
      .await;
  assert_eq!(page.status(), StatusCode::OK);
  let html = body_text(page).await;
  assert!(html.contains("Directories"));
  assert!(html.contains("Groups"));
  assert!(html.contains("Permissions"));
  let csrf = csrf_token(&html);

  // Create a directory and a group, then add the member to the group.
  let mk = |uri: String, body: String| {
    let state = state.clone();
    let cookie = cookie.clone();
    async move {
      let resp = post_form(state, &uri, &body, Some(&cookie)).await;
      assert_eq!(resp.status(), StatusCode::SEE_OTHER, "{uri}");
    }
  };
  mk(
    format!("/app/organizations/{org}/directories"),
    format!("csrf={csrf}&name=Folder&parent_id="),
  )
  .await;
  let dir = repo::directories::list_for_org(pool, &tenant, &org).await.unwrap()
    [0]
    .id
    .clone();
  mk(
    format!("/app/organizations/{org}/groups"),
    format!("csrf={csrf}&name=Editors"),
  )
  .await;
  let group = repo::groups::list_for_org(pool, &tenant, &org).await.unwrap()[0]
    .id
    .clone();
  mk(
    format!("/app/organizations/{org}/groups/{group}/members"),
    format!("csrf={csrf}&email=member@example.test"),
  )
  .await;
  mk(
    format!("/app/organizations/{org}/documents/doc1/directory"),
    format!("csrf={csrf}&directory_id={dir}"),
  )
  .await;

  // Tighten the org default to none: every non-owner member loses access.
  mk(
    format!("/app/organizations/{org}/default-access"),
    format!("csrf={csrf}&default_access=none"),
  )
  .await;
  assert_eq!(access(&state, &tenant, &member).await, AccessLevel::None);
  assert_eq!(access(&state, &tenant, &other).await, AccessLevel::None);

  // Grant the group read/write on the directory: the group member inherits it,
  // the non-member stays denied.
  mk(
    format!("/app/organizations/{org}/permissions"),
    format!("csrf={csrf}&subject=group:{group}&target=directory:{dir}&access=read_write"),
  )
  .await;
  assert_eq!(access(&state, &tenant, &member).await, AccessLevel::ReadWrite);
  assert_eq!(access(&state, &tenant, &other).await, AccessLevel::None);
}

async fn access(
  state: &hygg_server::state::AppState,
  tenant: &str,
  user_id: &str,
) -> AccessLevel {
  repo::access::library_for_hash(
    &state.db.conn,
    state.entitlements.as_ref(),
    tenant,
    user_id,
    false,
    true,
    None,
    "doc1",
  )
  .await
  .unwrap()
}
