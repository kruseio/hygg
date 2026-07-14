use axum::http::StatusCode;
use hygg_server::repo;
use serde_json::Value;

use crate::helpers::*;

/// The reworked home: server-side pagination via the lazy-load endpoint, plus
/// filter / search / tag narrowing.
#[tokio::test]
async fn home_paginates_filters_searches_and_tags() {
  let (_dir, state) = migrated_state().await;
  let tenant = seed_admin_and_user(&state).await;
  let pool = &state.db.conn;
  let reader = repo::users::find_by_email(pool, &tenant, "reader@example.test")
    .await
    .unwrap()
    .unwrap();
  // 26 personal documents — one more than a page (24) — to exercise paging.
  for i in 0..26 {
    repo::books::upsert(
      pool,
      &tenant,
      &reader.id,
      &repo::books::BookInput {
        content_hash: &format!("doc-{i}"),
        title: &format!("Doc {i}"),
        author: "",
        format: "txt",
        size_bytes: 1,
      },
    )
    .await
    .unwrap();
  }
  // One organization document.
  let org = repo::organizations::create(pool, &tenant, "Team", &reader.id)
    .await
    .unwrap();
  repo::books::upsert(
    pool,
    &tenant,
    &reader.id,
    &repo::books::BookInput {
      content_hash: "shared",
      title: "Shared Doc",
      author: "",
      format: "txt",
      size_bytes: 1,
    },
  )
  .await
  .unwrap();
  repo::books::move_to_organization(
    pool,
    &tenant,
    &reader.id,
    "shared",
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

  // First page is server-rendered and advertises the next offset.
  let home = get(state.clone(), "/app/home", Some(&cookie)).await;
  assert_eq!(home.status(), StatusCode::OK);
  let html = body_text(home).await;
  assert!(html.contains("library-controls"));
  assert!(html.contains(r#"id="library-sentinel" data-next="24""#));
  let csrf = csrf_token(&html);

  // The lazy-load endpoint returns the rest and signals the end.
  let frag =
    get(state.clone(), "/app/home/library?offset=24", Some(&cookie)).await;
  assert_eq!(frag.status(), StatusCode::OK);
  let page: Value = serde_json::from_str(&body_text(frag).await).unwrap();
  assert!(page["next"].is_null(), "no page after the last");
  assert!(page["cards"].as_str().unwrap().contains("book-card"));

  // Filter: organization-only shows the shared doc, not the personal ones.
  let org_only =
    body_text(get(state.clone(), "/app/home?filter=org", Some(&cookie)).await)
      .await;
  assert!(org_only.contains("Shared Doc"));
  assert!(!org_only.contains(">Doc 0<"));
  let owned_only = body_text(
    get(state.clone(), "/app/home?filter=owned", Some(&cookie)).await,
  )
  .await;
  assert!(!owned_only.contains("Shared Doc"));

  // Search by title.
  let searched =
    body_text(get(state.clone(), "/app/home?q=Doc+7", Some(&cookie)).await)
      .await;
  assert!(searched.contains(">Doc 7<"));
  assert!(!searched.contains(">Doc 8<"));

  // Tag a document, then filter by that tag.
  let tagged = post_form(
    state.clone(),
    "/app/books/doc-3/tags",
    &format!("csrf={csrf}&tag=favorite"),
    Some(&cookie),
  )
  .await;
  assert_eq!(tagged.status(), StatusCode::SEE_OTHER);
  let by_tag = body_text(
    get(state.clone(), "/app/home?tag=favorite", Some(&cookie)).await,
  )
  .await;
  assert!(by_tag.contains(">Doc 3<"));
  assert!(!by_tag.contains(">Doc 4<"));
  // The tag is visible on the document.
  assert!(by_tag.contains(r#"<span class="tag">favorite"#));
}
