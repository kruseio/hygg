use super::*;

/// `GET /app/organizations` — the organizations the caller can manage (those
/// they own; admins see all).
pub(crate) async fn organizations_manage_index(
  State(state): State<AppState>,
  headers: HeaderMap,
) -> Response {
  let Some(user) = current_user(&state, &headers).await else {
    return Redirect::to("/login").into_response();
  };
  let manageable: Vec<(String, String)> = if user.is_admin() {
    repo::organizations::list_for_tenant(&state.db.conn, &user.tenant_id)
      .await
      .unwrap_or_default()
      .into_iter()
      .map(|org| (org.id, org.name))
      .collect()
  } else {
    repo::organizations::list_for_user(
      &state.db.conn,
      &user.tenant_id,
      &user.user_id,
    )
    .await
    .unwrap_or_default()
    .into_iter()
    .filter(|m| m.role == "owner")
    .map(|m| (m.id, m.name))
    .collect()
  };
  let mut rows = String::new();
  for (id, name) in &manageable {
    rows.push_str(&format!(
      r#"<li><a href="/app/organizations/{}">{}</a></li>"#,
      esc(id),
      esc(name),
    ));
  }
  if rows.is_empty() {
    rows.push_str(
      r#"<li class="muted">You don't manage any organizations.</li>"#,
    );
  }
  page(
    "Organizations",
    Some(&user),
    format!(
      r#"<section class="panel"><h2>Organizations you manage</h2>
        <ul class="plain">{rows}</ul></section>"#
    ),
  )
}

/// `GET /app/organizations/{id}` — owner/admin management page.
pub(crate) async fn organization_manage_page(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(id): Path<String>,
) -> Response {
  let (user, org) = match require_org_manager(&state, &headers, &id).await {
    Ok(value) => value,
    Err(response) => return response,
  };
  let pool = &state.db.conn;
  let tenant = &user.tenant_id;
  let members = repo::organizations::list_members(pool, tenant, &id)
    .await
    .unwrap_or_default();
  let directories = repo::directories::list_for_org(pool, tenant, &id)
    .await
    .unwrap_or_default();
  let group_rows =
    repo::groups::list_for_org(pool, tenant, &id).await.unwrap_or_default();
  let mut groups = Vec::with_capacity(group_rows.len());
  for group in group_rows {
    let members = repo::groups::list_members(pool, tenant, &group.id)
      .await
      .unwrap_or_default();
    groups.push((group, members));
  }
  let books = repo::books::list_for_organization(pool, tenant, &id)
    .await
    .unwrap_or_default();
  let perms = repo::permissions::list_for_org(pool, tenant, &id)
    .await
    .unwrap_or_default();
  // The web extension's injected panels; none unless an override adds any.
  let ext_panels = state.web_ext.org_panels(&user, &org).await;
  page(
    "Manage organization",
    Some(&user),
    org_manage_content(
      &user,
      &org,
      &ext_panels,
      &members,
      &directories,
      &groups,
      &books,
      &perms,
    ),
  )
}

/// Live seat/storage/device usage for an organization.
pub(crate) async fn org_usage(
  state: &AppState,
  tenant_id: &str,
  organization_id: &str,
) -> OrgUsage {
  let pool = &state.db.conn;
  OrgUsage {
    seats: repo::organizations::count_members(pool, tenant_id, organization_id)
      .await
      .unwrap_or(0),
    storage: repo::books::storage_used_by_org(pool, tenant_id, organization_id)
      .await
      .unwrap_or(0),
    devices: repo::devices::count_for_org(pool, tenant_id, organization_id)
      .await
      .unwrap_or(0),
  }
}
