use super::*;

/// Admin-only organization detail + management page. Admins may manage any org
/// in the tenant regardless of membership.
pub(crate) async fn organization_page(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(organization_id): Path<String>,
) -> Response {
  let user = match require_admin_response(&state, &headers).await {
    Ok(user) => user,
    Err(response) => return response,
  };
  let Some(org) = repo::organizations::find_by_id(
    &state.db.conn,
    &user.tenant_id,
    &organization_id,
  )
  .await
  .ok()
  .flatten() else {
    return error_page(StatusCode::NOT_FOUND, "Organization not found");
  };
  let members = repo::organizations::list_members(
    &state.db.conn,
    &user.tenant_id,
    &organization_id,
  )
  .await
  .unwrap_or_default();
  let books = repo::books::list_for_organization(
    &state.db.conn,
    &user.tenant_id,
    &organization_id,
  )
  .await
  .unwrap_or_default();
  // The web extension's injected panels; none unless an override adds any.
  let ext_panels = state.web_ext.org_panels(&user, &org).await;
  page(
    "Organization",
    Some(&user),
    organization_content(&user, &org, &ext_panels, &members, &books),
  )
}

pub(crate) async fn organization_settings_post(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(organization_id): Path<String>,
  Form(form): Form<HashMap<String, String>>,
) -> Response {
  let user = match require_admin_response(&state, &headers).await {
    Ok(user) => user,
    Err(response) => return response,
  };
  if !csrf_ok(&user, &form) {
    return error_page(StatusCode::FORBIDDEN, "Invalid CSRF token");
  }
  let name = field(&form, "name");
  if !name.is_empty() {
    let _ = repo::organizations::rename(
      &state.db.conn,
      &user.tenant_id,
      &organization_id,
      &name,
    )
    .await;
  }
  let access = normalized_access(&field(&form, "default_access"));
  let _ = repo::organizations::set_default_access(
    &state.db.conn,
    &user.tenant_id,
    &organization_id,
    access,
  )
  .await;
  redirect_to_org(&organization_id)
}

pub(crate) async fn organization_delete_post(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(organization_id): Path<String>,
  Form(form): Form<HashMap<String, String>>,
) -> Response {
  let user = match require_admin_response(&state, &headers).await {
    Ok(user) => user,
    Err(response) => return response,
  };
  if !csrf_ok(&user, &form) {
    return error_page(StatusCode::FORBIDDEN, "Invalid CSRF token");
  }
  let _ = repo::organizations::delete(
    &state.db.conn,
    &user.tenant_id,
    &organization_id,
  )
  .await;
  Redirect::to("/app/admin/organizations").into_response()
}

pub(crate) async fn organization_member_post(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(organization_id): Path<String>,
  Form(form): Form<HashMap<String, String>>,
) -> Response {
  let user = match require_admin_response(&state, &headers).await {
    Ok(user) => user,
    Err(response) => return response,
  };
  if !csrf_ok(&user, &form) {
    return error_page(StatusCode::FORBIDDEN, "Invalid CSRF token");
  }
  if !organization_member_limit_allows(
    &state,
    &user.tenant_id,
    &organization_id,
  )
  .await
  {
    return error_page(
      StatusCode::FORBIDDEN,
      "Organization seat limit reached",
    );
  }
  let email = field(&form, "email").to_lowercase();
  let role = org_member_role(&field(&form, "role"));
  let Ok(Some(target)) =
    repo::users::find_by_email(&state.db.conn, &user.tenant_id, &email).await
  else {
    return error_page(StatusCode::NOT_FOUND, "User not found");
  };
  let _ = repo::organizations::add_member(
    &state.db.conn,
    &user.tenant_id,
    &organization_id,
    &target.id,
    role,
  )
  .await;
  check_org(&state, &user.tenant_id, &organization_id).await;
  redirect_to_org(&organization_id)
}

pub(crate) async fn organization_member_role_post(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path((organization_id, user_id)): Path<(String, String)>,
  Form(form): Form<HashMap<String, String>>,
) -> Response {
  let user = match require_admin_response(&state, &headers).await {
    Ok(user) => user,
    Err(response) => return response,
  };
  if !csrf_ok(&user, &form) {
    return error_page(StatusCode::FORBIDDEN, "Invalid CSRF token");
  }
  let role = org_member_role(&field(&form, "role"));
  if role != "owner"
    && would_orphan_org(&state, &user.tenant_id, &organization_id, &user_id)
      .await
  {
    return error_page(StatusCode::FORBIDDEN, "Cannot demote the last owner");
  }
  let _ = repo::organizations::set_member_role(
    &state.db.conn,
    &user.tenant_id,
    &organization_id,
    &user_id,
    role,
  )
  .await;
  redirect_to_org(&organization_id)
}

pub(crate) async fn organization_member_remove_post(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path((organization_id, user_id)): Path<(String, String)>,
  Form(form): Form<HashMap<String, String>>,
) -> Response {
  let user = match require_admin_response(&state, &headers).await {
    Ok(user) => user,
    Err(response) => return response,
  };
  if !csrf_ok(&user, &form) {
    return error_page(StatusCode::FORBIDDEN, "Invalid CSRF token");
  }
  if would_orphan_org(&state, &user.tenant_id, &organization_id, &user_id).await
  {
    return error_page(StatusCode::FORBIDDEN, "Cannot remove the last owner");
  }
  let _ = repo::organizations::remove_member(
    &state.db.conn,
    &user.tenant_id,
    &organization_id,
    &user_id,
  )
  .await;
  redirect_to_org(&organization_id)
}

/// True if `user_id` is currently the org's only owner (so demoting/removing
/// them would leave the org with no owner).
async fn would_orphan_org(
  state: &AppState,
  tenant_id: &str,
  organization_id: &str,
  user_id: &str,
) -> bool {
  let is_owner = repo::organizations::user_role(
    &state.db.conn,
    tenant_id,
    organization_id,
    user_id,
  )
  .await
  .ok()
  .flatten()
  .as_deref()
    == Some("owner");
  if !is_owner {
    return false;
  }
  let owners = repo::organizations::count_owners(
    &state.db.conn,
    tenant_id,
    organization_id,
  )
  .await
  .unwrap_or(0);
  owners <= 1
}

fn org_member_role(value: &str) -> &'static str {
  if value == "owner" { "owner" } else { "member" }
}

fn redirect_to_org(organization_id: &str) -> Response {
  Redirect::to(&format!("/app/admin/organizations/{organization_id}"))
    .into_response()
}
