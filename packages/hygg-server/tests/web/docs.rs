//! The public `/docs` documentation center: the index grid, a rendered page
//! with heading anchors and tables, full-text search that links to the exact
//! section, and a 404 for unknown pages. All routes are public (no session).

use axum::http::{StatusCode, header};

use crate::helpers::*;

#[tokio::test]
async fn docs_index_lists_every_page_and_offers_search() {
  let (_dir, state) = migrated_state().await;

  let resp = get(state, "/docs", None).await;
  assert_eq!(resp.status(), StatusCode::OK);
  let html = body_text(resp).await;

  // A card links to each embedded page.
  for slug in [
    "getting-started",
    "text-to-speech",
    "reference",
    "development",
    "responsible-ai",
    "benchmark",
  ] {
    assert!(
      html.contains(&format!(r#"href="/docs/{slug}""#)),
      "index should link to /docs/{slug}"
    );
  }
  // The search box and the public "Resources" nav are present.
  assert!(html.contains(r#"action="/docs/search""#));
  assert!(html.contains(r#"href="/docs""#));
}

#[tokio::test]
async fn docs_page_renders_headings_tables_and_code() {
  let (_dir, state) = migrated_state().await;

  let resp = get(state, "/docs/text-to-speech", None).await;
  assert_eq!(resp.status(), StatusCode::OK);
  let html = body_text(resp).await;

  // Headings carry stable slug ids that anchor deep links.
  assert!(html.contains(r#"id="voice-blending""#));
  assert!(html.contains(r#"id="voice-configuration""#));
  // GitHub-flavoured tables and fenced code render.
  assert!(html.contains("<table>"));
  assert!(html.contains(r#"class="language-sh""#));
  // The "On this page" table of contents is built for multi-heading pages.
  assert!(html.contains(r#"class="doc-toc-item""#));
}

#[tokio::test]
async fn docs_search_links_to_the_exact_section() {
  let (_dir, state) = migrated_state().await;

  let resp = get(state, "/docs/search?q=blending", None).await;
  assert_eq!(resp.status(), StatusCode::OK);
  let html = body_text(resp).await;

  // The hit links to the page with the query preserved (for on-page highlight)
  // and the section anchor (for the jump to the exact position).
  assert!(
    html.contains(r#"href="/docs/text-to-speech?q=blending#voice-blending""#),
    "search should deep-link to the matching section"
  );
}

#[tokio::test]
async fn docs_search_matches_text_inside_code_blocks() {
  let (_dir, state) = migrated_state().await;

  // `af_nicole` only appears inside fenced code, so this proves code content is
  // indexed too.
  let resp = get(state, "/docs/search?q=af_nicole", None).await;
  assert_eq!(resp.status(), StatusCode::OK);
  let html = body_text(resp).await;
  assert!(html.contains("<mark>af_nicole</mark>"));
}

#[tokio::test]
async fn docs_search_json_feeds_the_typeahead_top_hits() {
  let (_dir, state) = migrated_state().await;

  let resp = get(state.clone(), "/docs/search.json?q=blending", None).await;
  assert_eq!(resp.status(), StatusCode::OK);
  // The typeahead consumes it as JSON.
  let ct = resp
    .headers()
    .get(header::CONTENT_TYPE)
    .and_then(|v| v.to_str().ok())
    .unwrap_or_default()
    .to_string();
  assert!(ct.starts_with("application/json"), "content-type was {ct}");

  let hits: serde_json::Value =
    serde_json::from_str(&body_text(resp).await).unwrap();
  let hits = hits.as_array().unwrap();
  assert!(!hits.is_empty());
  // Each hit deep-links to the exact section (query preserved for highlight)
  // and carries its page/section crumb.
  assert!(hits.iter().any(|h| {
    h["href"] == "/docs/text-to-speech?q=blending#voice-blending"
      && h["page"] == "Text to Speech"
  }));

  // A body match carries its snippet as pre-highlighted HTML, unescaped through
  // JSON so the dropdown can render the `<mark>` directly (`af_nicole` only
  // appears inside fenced code, proving code content is fed too).
  let resp = get(state, "/docs/search.json?q=af_nicole", None).await;
  assert!(body_text(resp).await.contains("<mark>af_nicole</mark>"));
}

#[tokio::test]
async fn docs_search_json_caps_results_at_five() {
  let (_dir, state) = migrated_state().await;

  // A near-ubiquitous term matches far more than five sections; the feed keeps
  // only the top five so the dropdown stays a short, navigable list.
  let resp = get(state, "/docs/search.json?q=the", None).await;
  assert_eq!(resp.status(), StatusCode::OK);
  let hits: serde_json::Value =
    serde_json::from_str(&body_text(resp).await).unwrap();
  assert_eq!(hits.as_array().unwrap().len(), 5);
}

#[tokio::test]
async fn docs_search_box_is_a_typeahead_combobox() {
  let (_dir, state) = migrated_state().await;

  let html = body_text(get(state, "/docs", None).await).await;
  // The combobox input, the listbox it fills as you type, and the JSON feed the
  // search-as-you-type script fetches.
  assert!(html.contains(r#"id="doc-search-input""#));
  assert!(html.contains(r#"role="listbox""#));
  assert!(html.contains("/docs/search.json"));
  // The plain-form fallback (no-JS) still posts to the full results page.
  assert!(html.contains(r#"action="/docs/search""#));
}

#[tokio::test]
async fn docs_page_with_query_injects_the_highlight_script() {
  let (_dir, state) = migrated_state().await;

  // Without a query, no client script; with one, the highlight/scroll script.
  let plain =
    body_text(get(state.clone(), "/docs/reference", None).await).await;
  assert!(!plain.contains("createTreeWalker"));

  let highlighted =
    body_text(get(state, "/docs/reference?q=pdftotext", None).await).await;
  assert!(highlighted.contains("createTreeWalker"));
}

#[tokio::test]
async fn docs_unknown_page_is_not_found() {
  let (_dir, state) = migrated_state().await;

  let resp = get(state, "/docs/does-not-exist", None).await;
  assert_eq!(resp.status(), StatusCode::NOT_FOUND);
  let html = body_text(resp).await;
  assert!(html.contains("Page not found"));
}
