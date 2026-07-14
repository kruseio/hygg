use super::*;

/// `POST /app/organizations/{id}/permissions` — grant `subject` (`user:<id>` or
/// `group:<id>`) the chosen access on `target` (`document:<hash>` or
/// `directory:<id>`).
pub(crate) async fn org_permission_set_post(
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
  let subject = field(&form, "subject");
  let target = field(&form, "target");
  let (Some((subject_type, subject_id)), Some((target_type, target_id))) =
    (subject.split_once(':'), target.split_once(':'))
  else {
    return redirect_manage(&id);
  };
  if !matches!(subject_type, "user" | "group")
    || !matches!(target_type, "document" | "directory")
  {
    return error_page(StatusCode::BAD_REQUEST, "Invalid permission");
  }
  let access = normalized_access(&field(&form, "access"));
  let _ = repo::permissions::set(
    &state.db.conn,
    &user.tenant_id,
    &id,
    subject_type,
    subject_id,
    target_type,
    target_id,
    access,
  )
  .await;
  redirect_manage(&id)
}

/// `POST /app/organizations/{id}/permissions/remove` — delete a grant.
pub(crate) async fn org_permission_remove_post(
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
  let subject = field(&form, "subject");
  let target = field(&form, "target");
  let (Some((subject_type, subject_id)), Some((target_type, target_id))) =
    (subject.split_once(':'), target.split_once(':'))
  else {
    return redirect_manage(&id);
  };
  let _ = repo::permissions::remove(
    &state.db.conn,
    &user.tenant_id,
    &id,
    subject_type,
    subject_id,
    target_type,
    target_id,
  )
  .await;
  redirect_manage(&id)
}
