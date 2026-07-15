use super::*;

pub(crate) async fn book_organization_post(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(content_hash): Path<String>,
  Form(form): Form<HashMap<String, String>>,
) -> Response {
  let user = match require_workspace_user_response(&state, &headers).await {
    Ok(user) => user,
    Err(response) => return response,
  };
  if !csrf_ok(&user, &form) {
    return error_page(StatusCode::FORBIDDEN, "Invalid CSRF token");
  }
  let org_id = field(&form, "organization_id");
  let organization_id = if org_id.is_empty() {
    None
  } else {
    if !repo::organizations::user_can_access(
      &state.db.conn,
      &user.tenant_id,
      &org_id,
      &user.user_id,
    )
    .await
    .unwrap_or(false)
    {
      return error_page(StatusCode::FORBIDDEN, "Organization not available");
    }
    Some(org_id)
  };
  let moved = repo::books::move_to_organization(
    &state.db.conn,
    &user.tenant_id,
    &user.user_id,
    &content_hash,
    organization_id.as_deref(),
  )
  .await
  .unwrap_or(false);
  if !moved {
    return error_page(StatusCode::NOT_FOUND, "Document not found");
  }
  Redirect::to("/app/home").into_response()
}

pub(crate) async fn organization_member_limit_allows(
  state: &AppState,
  tenant_id: &str,
  organization_id: &str,
) -> bool {
  if repo::organizations::find_by_id(&state.db.conn, tenant_id, organization_id)
    .await
    .ok()
    .flatten()
    .is_none()
  {
    return false;
  }
  // The seat budget comes from the entitlements hook, which reports it
  // unlimited unless overridden (a limit of 0 also means unlimited).
  let Some(seats) = state
    .entitlements
    .org_limits(crate::ext::OrgCtx { tenant_id, organization_id })
    .await
    .seats
    .filter(|&seats| seats > 0)
  else {
    return true;
  };
  let current = repo::organizations::count_members(
    &state.db.conn,
    tenant_id,
    organization_id,
  )
  .await
  .unwrap_or(i64::MAX);
  current < seats
}
