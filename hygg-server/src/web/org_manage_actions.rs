use super::*;

pub(crate) fn redirect_manage(id: &str) -> Response {
  Redirect::to(&format!("/app/organizations/{id}")).into_response()
}

/// True if the group belongs to the organization (guards cross-org edits).
pub(crate) async fn group_in_org(
  state: &AppState,
  tenant_id: &str,
  org_id: &str,
  group_id: &str,
) -> bool {
  repo::groups::list_for_org(&state.db.conn, tenant_id, org_id)
    .await
    .unwrap_or_default()
    .iter()
    .any(|group| group.id == group_id)
}

pub(crate) async fn org_default_access_post(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(id): Path<String>,
  Form(form): Form<HashMap<String, String>>,
) -> Response {
  let (user, _) = match require_org_manager(&state, &headers, &id).await {
    Ok(value) => value,
    Err(response) => return response,
  };
  if !csrf_ok(&user, &form) {
    return error_page(StatusCode::FORBIDDEN, "Invalid CSRF token");
  }
  let access = normalized_access(&field(&form, "default_access"));
  let _ = repo::organizations::set_default_access(
    &state.db.conn,
    &user.tenant_id,
    &id,
    access,
  )
  .await;
  redirect_manage(&id)
}

pub(crate) async fn org_directory_create_post(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(id): Path<String>,
  Form(form): Form<HashMap<String, String>>,
) -> Response {
  let (user, _) = match require_org_manager(&state, &headers, &id).await {
    Ok(value) => value,
    Err(response) => return response,
  };
  if !csrf_ok(&user, &form) {
    return error_page(StatusCode::FORBIDDEN, "Invalid CSRF token");
  }
  let name = field(&form, "name");
  if name.is_empty() {
    return redirect_manage(&id);
  }
  let parent = field(&form, "parent_id");
  let parent = if parent.is_empty() { None } else { Some(parent) };
  let _ = repo::directories::create(
    &state.db.conn,
    &user.tenant_id,
    &id,
    parent.as_deref(),
    &name,
  )
  .await;
  redirect_manage(&id)
}

pub(crate) async fn org_document_directory_post(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path((id, content_hash)): Path<(String, String)>,
  Form(form): Form<HashMap<String, String>>,
) -> Response {
  let (user, _) = match require_org_manager(&state, &headers, &id).await {
    Ok(value) => value,
    Err(response) => return response,
  };
  if !csrf_ok(&user, &form) {
    return error_page(StatusCode::FORBIDDEN, "Invalid CSRF token");
  }
  let in_org =
    repo::books::access_meta(&state.db.conn, &user.tenant_id, &content_hash)
      .await
      .ok()
      .flatten()
      .and_then(|meta| meta.organization_id)
      .as_deref()
      == Some(id.as_str());
  if !in_org {
    return error_page(
      StatusCode::FORBIDDEN,
      "Document not in this organization",
    );
  }
  let directory = field(&form, "directory_id");
  let directory = if directory.is_empty() {
    None
  } else {
    let dirs =
      repo::directories::list_for_org(&state.db.conn, &user.tenant_id, &id)
        .await
        .unwrap_or_default();
    if !dirs.iter().any(|dir| dir.id == directory) {
      return error_page(StatusCode::BAD_REQUEST, "Unknown directory");
    }
    Some(directory)
  };
  let _ = repo::books::set_directory(
    &state.db.conn,
    &user.tenant_id,
    &content_hash,
    directory.as_deref(),
  )
  .await;
  redirect_manage(&id)
}

pub(crate) async fn org_group_create_post(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(id): Path<String>,
  Form(form): Form<HashMap<String, String>>,
) -> Response {
  let (user, _) = match require_org_manager(&state, &headers, &id).await {
    Ok(value) => value,
    Err(response) => return response,
  };
  if !csrf_ok(&user, &form) {
    return error_page(StatusCode::FORBIDDEN, "Invalid CSRF token");
  }
  let name = field(&form, "name");
  if !name.is_empty() {
    let _ =
      repo::groups::create(&state.db.conn, &user.tenant_id, &id, &name).await;
  }
  redirect_manage(&id)
}

pub(crate) async fn org_group_member_add_post(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path((id, group_id)): Path<(String, String)>,
  Form(form): Form<HashMap<String, String>>,
) -> Response {
  let (user, _) = match require_org_manager(&state, &headers, &id).await {
    Ok(value) => value,
    Err(response) => return response,
  };
  if !csrf_ok(&user, &form) {
    return error_page(StatusCode::FORBIDDEN, "Invalid CSRF token");
  }
  if !group_in_org(&state, &user.tenant_id, &id, &group_id).await {
    return error_page(StatusCode::NOT_FOUND, "Group not found");
  }
  let email = field(&form, "email").to_lowercase();
  let Ok(Some(target)) =
    repo::users::find_by_email(&state.db.conn, &user.tenant_id, &email).await
  else {
    return error_page(StatusCode::NOT_FOUND, "User not found");
  };
  if !repo::organizations::user_can_access(
    &state.db.conn,
    &user.tenant_id,
    &id,
    &target.id,
  )
  .await
  .unwrap_or(false)
  {
    return error_page(
      StatusCode::FORBIDDEN,
      "User is not an organization member",
    );
  }
  let _ = repo::groups::add_member(
    &state.db.conn,
    &user.tenant_id,
    &group_id,
    &target.id,
  )
  .await;
  redirect_manage(&id)
}

pub(crate) async fn org_group_member_remove_post(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path((id, group_id, user_id)): Path<(String, String, String)>,
  Form(form): Form<HashMap<String, String>>,
) -> Response {
  let (user, _) = match require_org_manager(&state, &headers, &id).await {
    Ok(value) => value,
    Err(response) => return response,
  };
  if !csrf_ok(&user, &form) {
    return error_page(StatusCode::FORBIDDEN, "Invalid CSRF token");
  }
  if !group_in_org(&state, &user.tenant_id, &id, &group_id).await {
    return error_page(StatusCode::NOT_FOUND, "Group not found");
  }
  let _ = repo::groups::remove_member(
    &state.db.conn,
    &user.tenant_id,
    &group_id,
    &user_id,
  )
  .await;
  redirect_manage(&id)
}
