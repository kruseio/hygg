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
async fn organization_documents_are_shared_but_progress_is_per_user() {
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
  let friend_hash = hash_password("friendpass123").unwrap();
  let friend_id = repo::users::insert(
    &state.db.conn,
    &tenant_id,
    "friend@example.test",
    "Friend",
    Some(&friend_hash),
    "user",
  )
  .await
  .unwrap();
  let org_id = repo::organizations::create(
    &state.db.conn,
    &tenant_id,
    "Reading Team",
    &reader.id,
  )
  .await
  .unwrap();
  repo::organizations::add_member(
    &state.db.conn,
    &tenant_id,
    &org_id,
    &friend_id,
    "member",
  )
  .await
  .unwrap();
  repo::books::upsert(
    &state.db.conn,
    &tenant_id,
    &reader.id,
    &repo::books::BookInput {
      content_hash: "book-shared",
      title: "Shared Book",
      author: "",
      format: "txt",
      size_bytes: 10,
    },
  )
  .await
  .unwrap();

  let reader_login = post_form(
    state.clone(),
    "/login",
    "email=reader%40example.test&password=readerpass123",
    None,
  )
  .await;
  let reader_cookie = session_cookie(&reader_login);
  let home = get(state.clone(), "/app/home", Some(&reader_cookie)).await;
  let csrf = csrf_token(&body_text(home).await);
  let moved = post_form(
    state.clone(),
    "/app/books/book-shared/organization",
    &format!("csrf={csrf}&organization_id={org_id}"),
    Some(&reader_cookie),
  )
  .await;
  assert_eq!(moved.status(), StatusCode::SEE_OTHER);

  let friend_login = post_form(
    state.clone(),
    "/login",
    "email=friend%40example.test&password=friendpass123",
    None,
  )
  .await;
  let friend_cookie = session_cookie(&friend_login);
  let friend_home = get(state.clone(), "/app/home", Some(&friend_cookie)).await;
  let friend_html = body_text(friend_home).await;
  assert!(friend_html.contains("Shared Book"));
  assert!(friend_html.contains("Organization"));
  // The shared document carries the organization icon + the org's name.
  assert!(friend_html.contains("org-chip"));
  assert!(friend_html.contains("Reading Team"));

  let reader_token =
    register_device_for(state.clone(), "reader@example.test", "readerpass123")
      .await;
  let friend_token =
    register_device_for(state.clone(), "friend@example.test", "friendpass123")
      .await;
  push_progress(
    state.clone(),
    &reader_token,
    "reader@example.test",
    "reader-progress",
    11,
  )
  .await;
  push_progress(
    state.clone(),
    &friend_token,
    "friend@example.test",
    "friend-progress",
    44,
  )
  .await;

  assert_eq!(
    pull_progress_offset(state.clone(), &reader_token, "reader@example.test")
      .await,
    11
  );
  assert_eq!(
    pull_progress_offset(state, &friend_token, "friend@example.test").await,
    44
  );
}

#[tokio::test]
async fn home_shows_storage_and_supports_two_stage_delete() {
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

  // Seed an owned book with stored document bytes.
  repo::books::upsert(
    &state.db.conn,
    &tenant_id,
    &reader.id,
    &repo::books::BookInput {
      content_hash: "store-book",
      title: "Stored Book",
      author: "",
      format: "pdf",
      size_bytes: 2048,
    },
  )
  .await
  .unwrap();
  let book_id =
    repo::books::find_id_by_hash(&state.db.conn, &tenant_id, "store-book")
      .await
      .unwrap()
      .unwrap();
  repo::blobs::put(
    &state.db.conn,
    &tenant_id,
    &book_id,
    &vec![7u8; 2048],
    "sha",
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

  // Home shows a storage meter and the per-document size; the card is a link
  // that opens a modal where the delete/move actions live.
  let home = get(state.clone(), "/app/home", Some(&cookie)).await;
  assert_eq!(home.status(), StatusCode::OK);
  let html = body_text(home).await;
  assert!(html.contains("storage-meter"));
  assert!(html.contains("<span>Storage</span>"));
  assert!(html.contains("Document 2.0 KB"));
  // Card links to the modal; the modal holds the advanced controls.
  assert!(html.contains(r##"href="#book-store-book""##));
  assert!(html.contains(r#"id="book-store-book""#));
  assert!(html.contains("/app/books/store-book/blob/delete"));
  assert!(html.contains("/app/books/store-book/delete"));
  assert!(html.contains("/app/books/store-book/organization"));
  let csrf = csrf_token(&html);

  // Delete the document only: the stored bytes go, the metadata row stays.
  let deleted_doc = post_form(
    state.clone(),
    "/app/books/store-book/blob/delete",
    &format!("csrf={csrf}"),
    Some(&cookie),
  )
  .await;
  assert_eq!(deleted_doc.status(), StatusCode::SEE_OTHER);
  assert!(
    repo::blobs::get(&state.db.conn, &tenant_id, &book_id)
      .await
      .unwrap()
      .is_none()
  );
  let books =
    repo::books::list_for_user(&state.db.conn, &tenant_id, &reader.id)
      .await
      .unwrap();
  assert_eq!(books.len(), 1, "metadata is retained after a document delete");

  // The metadata-only card now offers only the metadata delete.
  let home2 = get(state.clone(), "/app/home", Some(&cookie)).await;
  let html2 = body_text(home2).await;
  assert!(html2.contains("Document not on server"));
  assert!(html2.contains("/app/books/store-book/delete"));
  assert!(!html2.contains("/app/books/store-book/blob/delete"));
  let csrf2 = csrf_token(&html2);

  // Delete the metadata: the book is gone entirely.
  let deleted_meta = post_form(
    state.clone(),
    "/app/books/store-book/delete",
    &format!("csrf={csrf2}"),
    Some(&cookie),
  )
  .await;
  assert_eq!(deleted_meta.status(), StatusCode::SEE_OTHER);
  let books_after =
    repo::books::list_for_user(&state.db.conn, &tenant_id, &reader.id)
      .await
      .unwrap();
  assert!(books_after.is_empty(), "metadata delete removes the book");
}
