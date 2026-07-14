use std::collections::{HashMap, HashSet};

use axum::extract::Query;
use serde::Deserialize;

use super::*;
use crate::repo::books::BookRow;

pub(crate) const PAGE_SIZE: usize = 24;

#[derive(Deserialize, Default, Clone)]
pub(crate) struct LibraryQuery {
  pub filter: Option<String>,
  pub q: Option<String>,
  pub tag: Option<String>,
  pub offset: Option<usize>,
}

/// One fetch of everything the library view needs for a user, so the home page
/// and the lazy-load endpoint share identical data.
pub(crate) struct LibraryData {
  pub books: Vec<BookRow>,
  /// content_hash -> (percentage, offset_line, updated_at)
  pub progress: HashMap<String, (f64, i64, i64)>,
  pub reading_seconds: HashMap<String, i64>,
  pub blob_sizes: HashMap<String, i64>,
  pub org_names: HashMap<String, String>,
  pub tags_by_book: HashMap<String, Vec<String>>,
  pub organizations: Vec<repo::organizations::OrganizationMembership>,
  /// content_hash -> (sharer email, access level) for documents shared *to*
  /// the user, so their library cards can show a share badge + unshare
  /// control.
  pub shares_in: HashMap<String, (String, String)>,
}

pub(crate) async fn gather(state: &AppState, user: &WebUser) -> LibraryData {
  let pool = &state.db.conn;
  let tenant = &user.tenant_id;
  let is_admin = user.is_admin();
  let entitled = user.personal_sync;
  let mut books = Vec::new();
  for book in repo::books::list_for_user(pool, tenant, &user.user_id)
    .await
    .unwrap_or_default()
  {
    let can_read = repo::access::library(
      pool,
      state.entitlements.as_ref(),
      tenant,
      &user.user_id,
      is_admin,
      entitled,
      None,
      &book.owner_user_id,
      book.organization_id.as_deref(),
      book.directory_id.as_deref(),
      &book.content_hash,
    )
    .await
    .map(|access| access.can_read())
    .unwrap_or(false);
    if can_read {
      books.push(book);
    }
  }
  let progress = repo::progress::list_for_user(pool, tenant, &user.user_id)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|p| (p.book_id, (p.percentage, p.offset_line, p.updated_at)))
    .collect();
  let reading_seconds =
    repo::reading::seconds_by_book(pool, tenant, &user.user_id)
      .await
      .unwrap_or_default()
      .into_iter()
      .collect();
  let blob_sizes =
    repo::books::blob_sizes_for_user(pool, tenant, &user.user_id)
      .await
      .unwrap_or_default()
      .into_iter()
      .collect();
  let shares_in = repo::shares::accepted_incoming(pool, tenant, &user.user_id)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|s| (s.content_hash, (s.from_email, s.access)))
    .collect();
  let organizations =
    repo::organizations::list_for_user(pool, tenant, &user.user_id)
      .await
      .unwrap_or_default();
  let org_names =
    organizations.iter().map(|o| (o.id.clone(), o.name.clone())).collect();
  let org_ids: Vec<String> =
    organizations.iter().map(|o| o.id.clone()).collect();
  let mut tags_by_book: HashMap<String, Vec<String>> = HashMap::new();
  for tag in
    repo::tags::visible_book_tags(pool, tenant, &user.user_id, &org_ids)
      .await
      .unwrap_or_default()
  {
    tags_by_book.entry(tag.content_hash).or_default().push(tag.name);
  }
  LibraryData {
    books,
    progress,
    reading_seconds,
    blob_sizes,
    org_names,
    tags_by_book,
    organizations,
    shares_in,
  }
}

/// The distinct tag names visible to the user, sorted, for the filter dropdown.
pub(crate) fn tag_names(data: &LibraryData) -> Vec<String> {
  let mut names: Vec<String> =
    data.tags_by_book.values().flatten().cloned().collect();
  names.sort();
  names.dedup();
  names
}

/// Filter (owned/org/all) + search + tag, then bucket-order: the user's 5
/// most-recent personal documents, then 5 most-recent organization documents,
/// then the remaining read documents by recency, then never-read documents by
/// creation/metadata time.
pub(crate) fn ordered<'a>(
  data: &'a LibraryData,
  filter: &str,
  q: &str,
  tag: &str,
) -> Vec<&'a BookRow> {
  let needle = q.trim().to_lowercase();
  let candidates: Vec<&BookRow> = data
    .books
    .iter()
    .filter(|book| matches_filter(book, filter))
    .filter(|book| tag.is_empty() || has_tag(data, book, tag))
    .filter(|book| needle.is_empty() || matches_search(data, book, &needle))
    .collect();

  let recency = |book: &BookRow| {
    data.progress.get(&book.content_hash).map(|(_, _, at)| *at)
  };
  let mut read: Vec<&BookRow> =
    candidates.iter().copied().filter(|b| recency(b).is_some()).collect();
  let mut unread: Vec<&BookRow> =
    candidates.iter().copied().filter(|b| recency(b).is_none()).collect();
  read.sort_by_key(|book| std::cmp::Reverse(recency(book)));
  unread.sort_by_key(|book| std::cmp::Reverse(book.updated_at));

  let mut ordered = Vec::with_capacity(candidates.len());
  let mut seen = HashSet::new();
  let featured =
    read.iter().copied().filter(|b| b.organization_id.is_none()).take(5).chain(
      read.iter().copied().filter(|b| b.organization_id.is_some()).take(5),
    );
  for book in featured.chain(read.iter().copied()).chain(unread) {
    if seen.insert(book.content_hash.as_str()) {
      ordered.push(book);
    }
  }
  ordered
}

fn matches_filter(book: &BookRow, filter: &str) -> bool {
  match filter {
    "owned" => book.organization_id.is_none(),
    "org" => book.organization_id.is_some(),
    _ => true,
  }
}

fn has_tag(data: &LibraryData, book: &BookRow, tag: &str) -> bool {
  data
    .tags_by_book
    .get(&book.content_hash)
    .is_some_and(|tags| tags.iter().any(|t| t == tag))
}

fn matches_search(data: &LibraryData, book: &BookRow, needle: &str) -> bool {
  book.title.to_lowercase().contains(needle)
    || book.author.to_lowercase().contains(needle)
    || book
      .file_name
      .as_deref()
      .is_some_and(|f| f.to_lowercase().contains(needle))
    || data.tags_by_book.get(&book.content_hash).is_some_and(|tags| {
      tags.iter().any(|t| t.to_lowercase().contains(needle))
    })
}

pub(crate) fn card_for<'a>(
  user: &'a WebUser,
  book: &'a BookRow,
  data: &'a LibraryData,
) -> BookCard<'a> {
  let (pct, _offset, last) = data
    .progress
    .get(&book.content_hash)
    .copied()
    .unwrap_or((0.0, 0, book.updated_at));
  BookCard {
    user,
    book,
    pct: pct.round().clamp(0.0, 100.0) as i64,
    secs: data.reading_seconds.get(&book.content_hash).copied().unwrap_or(0),
    last_read: last,
    doc_bytes: data.blob_sizes.get(&book.content_hash).copied().unwrap_or(0),
    meta_bytes: metadata_bytes(book),
    has_blob: data.blob_sizes.contains_key(&book.content_hash),
    org_name: book.organization_id.as_deref().map(|id| {
      data.org_names.get(id).map(|s| s.as_str()).unwrap_or("Organization")
    }),
    tags: data
      .tags_by_book
      .get(&book.content_hash)
      .map(|v| v.as_slice())
      .unwrap_or(&[]),
    organizations: &data.organizations,
    share: data.shares_in.get(&book.content_hash),
  }
}

/// `GET /app/home/library` — a JSON page of rendered cards + modals for lazy
/// loading: `{ cards, modals, next }` where `next` is the offset to request
/// next (or null at the end).
pub(crate) async fn library_fragment(
  State(state): State<AppState>,
  headers: HeaderMap,
  Query(query): Query<LibraryQuery>,
) -> Response {
  let user = match require_workspace_user_response(&state, &headers).await {
    Ok(user) => user,
    Err(response) => return response,
  };
  let data = gather(&state, &user).await;
  let filter = query.filter.as_deref().unwrap_or("all");
  let q = query.q.as_deref().unwrap_or("");
  let tag = query.tag.as_deref().unwrap_or("");
  let offset = query.offset.unwrap_or(0);
  let ordered = ordered(&data, filter, q, tag);
  let (mut cards, mut modals) = (String::new(), String::new());
  for book in ordered.iter().skip(offset).take(PAGE_SIZE) {
    let (card, modal) = render_book(&card_for(&user, book, &data));
    cards.push_str(&card);
    modals.push_str(&modal);
  }
  let next = (offset + PAGE_SIZE < ordered.len()).then_some(offset + PAGE_SIZE);
  Json(json!({ "cards": cards, "modals": modals, "next": next }))
    .into_response()
}
