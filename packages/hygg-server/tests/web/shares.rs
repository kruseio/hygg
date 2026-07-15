//! Peer document sharing: the outbox/inbox web flow — an accepted share
//! surfaces in the recipient's library (the `/api/v1/books` list every client
//! reads), the recipient keeps their own reading progress, revoking (owner) and
//! unsharing (recipient) both remove access, and sharing to an unknown user is
//! rejected. Progress-isolation on org documents lives in `shares_progress`.

use axum::http::StatusCode;
use hygg_server::repo;

use crate::helpers::*;
use crate::shares_common::*;

#[tokio::test]
async fn share_accept_surfaces_in_recipient_library_with_separate_progress() {
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
  own_book(&state, &tenant, &reader.id, "novel").await;

  // The reader shares the document with the friend (read-only).
  let reader_cookie =
    login(state.clone(), "reader@example.test", "readerpass123").await;
  let csrf = csrf_token(
    &body_text(get(state.clone(), "/app/shares", Some(&reader_cookie)).await)
      .await,
  );
  let created = post_form(
    state.clone(),
    "/app/shares",
    &format!(
      "csrf={csrf}&content_hash=novel&email=friend%40example.test&access=read"
    ),
    Some(&reader_cookie),
  )
  .await;
  assert_eq!(created.status(), StatusCode::SEE_OTHER);

  let reader_token =
    register_device_for(state.clone(), "reader@example.test", "readerpass123")
      .await;
  let friend_token =
    register_device_for(state.clone(), "friend@example.test", "friendpass123")
      .await;

  // Before accepting, the friend cannot see the document.
  assert!(
    !books_contains(
      state.clone(),
      &friend_token,
      "friend@example.test",
      "novel"
    )
    .await
  );

  // The friend accepts the pending inbox share.
  let friend_cookie =
    login(state.clone(), "friend@example.test", "friendpass123").await;
  let inbox =
    repo::shares::list_inbox(pool, &tenant, &friend.id).await.unwrap();
  assert_eq!(inbox.len(), 1);
  let csrf_f = csrf_token(
    &body_text(get(state.clone(), "/app/shares", Some(&friend_cookie)).await)
      .await,
  );
  let accept = post_form(
    state.clone(),
    &format!("/app/shares/{}/accept", inbox[0].id),
    &format!("csrf={csrf_f}"),
    Some(&friend_cookie),
  )
  .await;
  assert_eq!(accept.status(), StatusCode::SEE_OTHER);

  // Now it appears in the friend's library (the list every client reads).
  assert!(
    books_contains(
      state.clone(),
      &friend_token,
      "friend@example.test",
      "novel"
    )
    .await
  );

  // Separate progress: owner at 10, recipient at 55 — neither sees the other's.
  push_progress_for(
    state.clone(),
    &reader_token,
    "reader@example.test",
    WEB_MACHINE,
    "novel",
    10,
  )
  .await;
  push_progress_for(
    state.clone(),
    &friend_token,
    "friend@example.test",
    WEB_MACHINE,
    "novel",
    55,
  )
  .await;
  assert_eq!(
    pull_offset_for(
      state.clone(),
      &reader_token,
      "reader@example.test",
      WEB_MACHINE,
      "novel"
    )
    .await,
    Some(10)
  );
  assert_eq!(
    pull_offset_for(
      state.clone(),
      &friend_token,
      "friend@example.test",
      WEB_MACHINE,
      "novel"
    )
    .await,
    Some(55)
  );

  // The reader revokes; the friend loses access.
  let revoke = post_form(
    state.clone(),
    &format!("/app/shares/{}/revoke", inbox[0].id),
    &format!("csrf={csrf}"),
    Some(&reader_cookie),
  )
  .await;
  assert_eq!(revoke.status(), StatusCode::SEE_OTHER);
  assert!(
    !books_contains(
      state.clone(),
      &friend_token,
      "friend@example.test",
      "novel"
    )
    .await
  );
}

#[tokio::test]
async fn recipient_can_unshare_a_document_from_their_library() {
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
  own_book(&state, &tenant, &reader.id, "novel").await;
  repo::shares::create(pool, &tenant, "novel", &reader.id, &friend.id, "read")
    .await
    .unwrap();
  let inbox =
    repo::shares::list_inbox(pool, &tenant, &friend.id).await.unwrap();
  repo::shares::accept(pool, &tenant, &inbox[0].id, &friend.id).await.unwrap();

  let friend_token =
    register_device_for(state.clone(), "friend@example.test", "friendpass123")
      .await;
  assert!(
    books_contains(
      state.clone(),
      &friend_token,
      "friend@example.test",
      "novel"
    )
    .await
  );

  // The recipient unshares the document from their own library.
  let cookie =
    login(state.clone(), "friend@example.test", "friendpass123").await;
  let csrf = csrf_token(
    &body_text(get(state.clone(), "/app/home", Some(&cookie)).await).await,
  );
  let resp = post_form(
    state.clone(),
    "/app/books/novel/unshare",
    &format!("csrf={csrf}"),
    Some(&cookie),
  )
  .await;
  assert_eq!(resp.status(), StatusCode::SEE_OTHER);
  assert!(
    !books_contains(
      state.clone(),
      &friend_token,
      "friend@example.test",
      "novel"
    )
    .await
  );
}

#[tokio::test]
async fn share_to_unknown_email_is_rejected() {
  let (_dir, state) = migrated_state().await;
  let tenant = seed_admin_and_user(&state).await;
  let pool = &state.db.conn;
  let reader = repo::users::find_by_email(pool, &tenant, "reader@example.test")
    .await
    .unwrap()
    .unwrap();
  own_book(&state, &tenant, &reader.id, "novel").await;
  let cookie =
    login(state.clone(), "reader@example.test", "readerpass123").await;
  let csrf = csrf_token(
    &body_text(get(state.clone(), "/app/shares", Some(&cookie)).await).await,
  );
  let resp = post_form(
    state.clone(),
    "/app/shares",
    &format!(
      "csrf={csrf}&content_hash=novel&email=nobody%40example.test&access=read"
    ),
    Some(&cookie),
  )
  .await;
  assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
