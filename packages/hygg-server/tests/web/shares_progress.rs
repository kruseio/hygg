//! Per-user reading-progress isolation on an organization document: two members
//! of the same org keep independent progress on the same document (progress is
//! keyed by user, never shared). The shared-document case is covered in
//! `shares`.

use hygg_server::repo;

use crate::helpers::*;
use crate::shares_common::*;

#[tokio::test]
async fn org_members_keep_separate_progress_on_a_shared_document() {
  let (_dir, state) = migrated_state().await;
  let tenant = seed_admin_and_user(&state).await;
  add_user(&state, &tenant, "friend@example.test").await;
  let pool = &state.db.conn;
  let reader = repo::users::find_by_email(pool, &tenant, "reader@example.test")
    .await
    .unwrap()
    .unwrap();
  let friend = repo::users::find_by_email(pool, &tenant, "friend@example.test")
    .await
    .unwrap()
    .unwrap();
  // A document owned by an organization both users belong to.
  own_book(&state, &tenant, &reader.id, "orgdoc").await;
  let org = repo::organizations::create(pool, &tenant, "Team", &reader.id)
    .await
    .unwrap();
  repo::books::move_to_organization(
    pool,
    &tenant,
    &reader.id,
    "orgdoc",
    Some(&org),
  )
  .await
  .unwrap();
  repo::organizations::add_member(pool, &tenant, &org, &friend.id, "member")
    .await
    .unwrap();

  let reader_token =
    register_device_for(state.clone(), "reader@example.test", "readerpass123")
      .await;
  let friend_token =
    register_device_for(state.clone(), "friend@example.test", "friendpass123")
      .await;
  // Both members see the org document.
  assert!(
    books_contains(
      state.clone(),
      &friend_token,
      "friend@example.test",
      "orgdoc"
    )
    .await
  );

  push_progress_for(
    state.clone(),
    &reader_token,
    "reader@example.test",
    WEB_MACHINE,
    "orgdoc",
    20,
  )
  .await;
  push_progress_for(
    state.clone(),
    &friend_token,
    "friend@example.test",
    WEB_MACHINE,
    "orgdoc",
    70,
  )
  .await;
  assert_eq!(
    pull_offset_for(
      state.clone(),
      &reader_token,
      "reader@example.test",
      WEB_MACHINE,
      "orgdoc"
    )
    .await,
    Some(20)
  );
  assert_eq!(
    pull_offset_for(
      state.clone(),
      &friend_token,
      "friend@example.test",
      WEB_MACHINE,
      "orgdoc"
    )
    .await,
    Some(70)
  );
}
