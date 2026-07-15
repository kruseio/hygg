//! HTTP handlers and view helpers for the docs center: the index grid, a
//! rendered page with its table of contents, and the search results list.

use super::*;

#[derive(Deserialize, Default)]
pub(crate) struct DocSearchQuery {
  #[serde(default)]
  pub q: Option<String>,
}

/// `/docs` — the documentation home: a search box and a card per page.
pub(crate) async fn docs_index(
  State(state): State<AppState>,
  headers: HeaderMap,
) -> Response {
  let user = current_user(&state, &headers).await;
  let cards: String = docs()
    .iter()
    .map(|doc| {
      format!(
        r#"<a class="doc-card" href="/docs/{slug}"><h2>{title}</h2><p>{desc}</p><span class="doc-card-more">Read <span aria-hidden="true">&rarr;</span></span></a>"#,
        slug = doc.slug,
        title = esc(doc.title),
        desc = doc_description(doc),
      )
    })
    .collect();
  let body = format!(
    r#"<section class="doc-hero"><p class="eyebrow">Documentation</p><h1>hygg docs</h1><p class="muted">Install, configure, and get the most out of the hygg reader.</p></section>
    {search}
    <div class="doc-card-grid">{cards}</div>"#,
    search = search_form(""),
  );
  page("Documentation · hygg", user.as_ref(), body)
}

/// `/docs/{slug}` — a rendered page with an "On this page" table of contents.
/// With `?q=` present, an inline script highlights the term and scrolls the
/// first match into view.
pub(crate) async fn docs_page(
  State(state): State<AppState>,
  headers: HeaderMap,
  axum::extract::Path(slug): axum::extract::Path<String>,
  axum::extract::Query(query): axum::extract::Query<DocSearchQuery>,
) -> Response {
  let user = current_user(&state, &headers).await;
  let Some(doc) = find_doc(&slug) else {
    return docs_not_found(user.as_ref());
  };
  let q = query.q.unwrap_or_default();
  let toc = toc_html(&doc.toc);
  let (layout_class, toc) = if toc.is_empty() {
    ("doc-layout doc-layout-single", String::new())
  } else {
    ("doc-layout", toc)
  };
  let highlight = if q.trim().is_empty() { "" } else { highlight_script() };
  let body = format!(
    r#"{back}
    {search}
    <div class="{layout_class}">
      {toc}
      <article class="doc-content">{content}</article>
    </div>
    {highlight}"#,
    back = back_link("/docs", "All documentation"),
    search = search_form(&q),
    content = doc.html,
  );
  page(&format!("{} · hygg docs", doc.title), user.as_ref(), body)
}

/// `/docs/search?q=` — results across every page, each linking to the exact
/// heading of the matching section.
pub(crate) async fn docs_search(
  State(state): State<AppState>,
  headers: HeaderMap,
  axum::extract::Query(query): axum::extract::Query<DocSearchQuery>,
) -> Response {
  let user = current_user(&state, &headers).await;
  let q = query.q.unwrap_or_default();
  let trimmed = q.trim();
  let results = if trimmed.is_empty() {
    r#"<p class="muted">Type a term above to search every documentation page.</p>"#.to_string()
  } else {
    search_results_html(trimmed)
  };
  let body = format!(
    r#"{back}
    {search}
    <section class="doc-results">{results}</section>"#,
    back = back_link("/docs", "All documentation"),
    search = search_form(&q),
  );
  page("Search · hygg docs", user.as_ref(), body)
}

/// `/docs/search.json?q=` — the typeahead's feed: the top 5 hits as a JSON
/// array, each with the deep-link `href` (query preserved for on-page
/// highlight, plus the section anchor), its `page`/`section` crumb, and the
/// pre-highlighted `snippet`. Public, like the rest of the docs center.
pub(crate) async fn docs_search_json(
  axum::extract::Query(query): axum::extract::Query<DocSearchQuery>,
) -> Response {
  let q = query.q.unwrap_or_default();
  let trimmed = q.trim();
  let encoded = encode_query(trimmed);
  let hits: Vec<_> = search_docs(trimmed)
    .into_iter()
    .take(5)
    .map(|hit| {
      let anchor = if hit.section_slug.is_empty() {
        String::new()
      } else {
        format!("#{}", hit.section_slug)
      };
      // Drop the crumb's section when it's the whole page or the lead, so the
      // dropdown shows just the page title in those cases.
      let section =
        if hit.section_slug.is_empty() || hit.section_title == hit.page_title {
          String::new()
        } else {
          hit.section_title.clone()
        };
      json!({
        "href": format!("/docs/{}?q={encoded}{anchor}", hit.page_slug),
        "page": hit.page_title,
        "section": section,
        "snippet": hit.snippet,
      })
    })
    .collect();
  Json(json!(hits)).into_response()
}

/// The results block for a non-empty query: the hit count and one linked card
/// per hit, or a "no matches" note.
fn search_results_html(query: &str) -> String {
  let hits = search_docs(query);
  if hits.is_empty() {
    return format!(
      r#"<p class="muted">No matches for &ldquo;{}&rdquo;.</p>"#,
      esc(query)
    );
  }
  let encoded = encode_query(query);
  let items: String =
    hits.iter().map(|hit| search_hit_html(hit, &encoded)).collect();
  format!(
    r#"<p class="muted">{count} result{plural} for &ldquo;{query}&rdquo;</p><div class="doc-hit-list">{items}</div>"#,
    count = hits.len(),
    plural = if hits.len() == 1 { "" } else { "s" },
    query = esc(query),
  )
}

/// A single search-result card, linking to the hit's page with the query
/// preserved (for on-page highlight) and the section anchor (for the jump).
fn search_hit_html(hit: &SearchHit, encoded_query: &str) -> String {
  let anchor = if hit.section_slug.is_empty() {
    String::new()
  } else {
    format!("#{}", hit.section_slug)
  };
  let crumb =
    if hit.section_slug.is_empty() || hit.section_title == hit.page_title {
      esc(hit.page_title)
    } else {
      format!(
        r#"{} <span class="doc-hit-sep" aria-hidden="true">&rsaquo;</span> {}"#,
        esc(hit.page_title),
        esc(&hit.section_title)
      )
    };
  format!(
    r#"<a class="doc-hit" href="/docs/{slug}?q={encoded_query}{anchor}"><span class="doc-hit-crumb">{crumb}</span><span class="doc-hit-snippet">{snippet}</span></a>"#,
    slug = hit.page_slug,
    snippet = hit.snippet,
  )
}

fn docs_not_found(user: Option<&WebUser>) -> Response {
  let list: String = docs()
    .iter()
    .map(|doc| {
      format!(r#"<li><a href="/docs/{}">{}</a></li>"#, doc.slug, esc(doc.title))
    })
    .collect();
  let body = format!(
    r#"<section class="panel"><h1>Page not found</h1><p class="muted">That documentation page doesn&rsquo;t exist. Browse the available pages:</p><ul>{list}</ul></section>"#,
  );
  (StatusCode::NOT_FOUND, page("Not found · hygg docs", user, body))
    .into_response()
}

/// The search box: a form that submits to the full-page `/docs/search` (the
/// no-JS fallback), enhanced by the typeahead into a combobox — the `<input>`
/// owns the `<ul>` listbox the script fills as you type.
fn search_form(query: &str) -> String {
  format!(
    r#"<form class="doc-search" action="/docs/search" method="get" role="search">
      <div class="doc-search-box">
        <input id="doc-search-input" type="search" name="q" value="{q}" placeholder="Search the docs…"
          aria-label="Search documentation" role="combobox" aria-expanded="false"
          aria-autocomplete="list" aria-controls="doc-search-menu" autocomplete="off">
        <ul id="doc-search-menu" class="doc-search-menu" role="listbox" aria-label="Search results" hidden></ul>
      </div>
      <button type="submit">Search</button>
    </form>
    {script}"#,
    q = esc(query),
    script = typeahead_script(),
  )
}

/// The "On this page" table of contents, or empty when a page has fewer than
/// two headings (nothing worth navigating).
fn toc_html(toc: &[TocItem]) -> String {
  if toc.len() < 2 {
    return String::new();
  }
  let min = toc.iter().map(|item| item.level).min().unwrap_or(1);
  let items: String = toc
    .iter()
    .map(|item| {
      format!(
        r##"<a class="doc-toc-item" style="padding-left:{indent}px" href="#{slug}">{title}</a>"##,
        indent = 10 + (item.level.saturating_sub(min) as usize) * 14,
        slug = item.slug,
        title = esc(&item.title),
      )
    })
    .collect();
  format!(
    r#"<aside class="doc-toc"><p class="doc-toc-head">On this page</p><nav>{items}</nav></aside>"#
  )
}

fn doc_description(doc: &RenderedDoc) -> String {
  doc
    .sections
    .iter()
    .find(|section| !section.text.trim().is_empty())
    .map(|section| lead_html(&section.text))
    .unwrap_or_default()
}
