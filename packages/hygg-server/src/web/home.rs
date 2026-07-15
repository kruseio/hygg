use super::*;

pub(crate) async fn dashboard(
  state: &AppState,
  user: WebUser,
  query: LibraryQuery,
) -> Response {
  let pool = &state.db.conn;
  let data = gather(state, &user).await;

  // Pending incoming shares, surfaced as a badge on the "Shared documents"
  // button that links to the outbox/inbox page.
  let inbox_pending =
    repo::shares::pending_inbox_count(pool, &user.tenant_id, &user.user_id)
      .await
      .unwrap_or(0);
  let shares_badge = if inbox_pending > 0 {
    format!(r#"<span class="count-badge">{inbox_pending}</span>"#)
  } else {
    String::new()
  };

  let storage_limit = state
    .entitlements
    .storage_limit(crate::ext::EntCtx {
      tenant_id: &user.tenant_id,
      user_id: &user.user_id,
      is_admin: user.is_admin(),
    })
    .await
    .filter(|&limit| limit > 0);
  // Revoked devices are hidden everywhere and don't count toward the stat.
  let devices: Vec<_> =
    repo::devices::list_for_user(pool, &user.tenant_id, &user.user_id)
      .await
      .unwrap_or_default()
      .into_iter()
      .filter(|d| d.revoked == 0)
      .collect();
  let total_seconds =
    repo::reading::total_seconds(pool, &user.tenant_id, &user.user_id)
      .await
      .unwrap_or(0);
  let active_days =
    repo::reading::active_days(pool, &user.tenant_id, &user.user_id)
      .await
      .unwrap_or_default();
  let streak =
    repo::reading::streak_for_days(&active_days, Utc::now().date_naive());

  let started = data.progress.values().filter(|(pct, _, _)| *pct > 0.0).count();
  let finished = data
    .progress
    .values()
    .filter(|(pct, _, _)| *pct >= FINISHED_PERCENTAGE)
    .count();
  let pages_read: i64 =
    data.progress.values().map(|(_, offset, _)| offset / LINES_PER_PAGE).sum();

  // Personal storage counts only the caller's own *personal* documents;
  // organization documents belong to a separate, org-owned pool.
  let mut document_bytes_used = 0i64;
  let mut metadata_bytes_used = 0i64;
  for book in &data.books {
    if book.owner_user_id == user.user_id && book.organization_id.is_none() {
      document_bytes_used +=
        data.blob_sizes.get(&book.content_hash).copied().unwrap_or(0);
      metadata_bytes_used += metadata_bytes(book);
    }
  }
  let mut org_usage = Vec::new();
  for org in &data.organizations {
    let used = repo::books::storage_used_by_org(pool, &user.tenant_id, &org.id)
      .await
      .unwrap_or(0);
    let limit = state
      .entitlements
      .org_limits(crate::ext::OrgCtx {
        tenant_id: &user.tenant_id,
        organization_id: &org.id,
      })
      .await
      .storage_bytes
      .filter(|&limit| limit > 0);
    org_usage.push((org.name.clone(), used, limit));
  }

  let filter = query.filter.as_deref().unwrap_or("all");
  let q = query.q.as_deref().unwrap_or("");
  let tag = query.tag.as_deref().unwrap_or("");
  let ordered = ordered(&data, filter, q, tag);
  let (mut cards, mut modals) = (String::new(), String::new());
  for book in ordered.iter().take(PAGE_SIZE) {
    let (card, modal) = render_book(&card_for(&user, book, &data));
    cards.push_str(&card);
    modals.push_str(&modal);
  }
  if cards.is_empty() {
    cards.push_str(
      r#"<p class="muted">No documents match. Connect the CLI and start reading.</p>"#,
    );
  }
  let next = if ordered.len() > PAGE_SIZE {
    PAGE_SIZE.to_string()
  } else {
    String::new()
  };
  let finished_label = format!("{finished} / {started}");

  page(
    "Home",
    Some(&user),
    format!(
      r#"<section class="grid">
        <div class="stat"><strong>{reading_time}</strong><span>reading time</span></div>
        <div class="stat"><strong>{documents}</strong><span>documents</span></div>
        <div class="stat"><strong>{finished}</strong><span>finished</span></div>
        <div class="stat"><strong>{pages}</strong><span>pages read</span></div>
        <div class="stat"><strong>{streak}</strong><span>day streak</span></div>
        <div class="stat"><strong>{devices}</strong><span>devices</span></div>
        <div class="stat"><strong>{orgs}</strong><span>organizations</span></div>
      </section>
      <section class="panel" id="library">
        <div class="panel-head"><h2>Your library</h2>
          <a class="button secondary" href="/app/shares">Shared documents{shares_badge}</a></div>
        {storage_meter}
        {controls}
        <div class="book-grid" id="library-items">{cards}</div>
        <div id="library-sentinel" data-next="{next}"></div>
      </section>
      {org_storage}
      <div id="library-modals">{modals}</div>
      {js}"#,
      reading_time = humanize_duration(total_seconds),
      shares_badge = shares_badge,
      documents = data.books.len(),
      finished = finished_label,
      pages = pages_read,
      streak = streak,
      devices = devices.len(),
      orgs = data.organizations.len(),
      storage_meter =
        storage_meter(document_bytes_used, metadata_bytes_used, storage_limit),
      org_storage = org_storage_panel(&org_usage),
      controls = library_controls(filter, q, tag, &tag_names(&data)),
      cards = cards,
      next = next,
      modals = modals,
      js = library_js(),
    ),
  )
}
