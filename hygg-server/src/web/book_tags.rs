use super::*;

/// `POST /app/books/{hash}/tags` — tag a document. Personal documents get a
/// private `user` tag; organization documents get an `org` tag shared with the
/// org. Requires read access to the document.
pub(crate) async fn book_tag_add_post(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(content_hash): Path<String>,
  Form(form): Form<HashMap<String, String>>,
) -> Response {
  let (user, scope_type, scope_id) =
    match tag_context(&state, &headers, &form, &content_hash).await {
      Ok(value) => value,
      Err(response) => return response,
    };
  let name = field(&form, "tag");
  let name = name.trim();
  if !name.is_empty() {
    let trimmed: String = name.chars().take(40).collect();
    if let Ok(tag_id) = repo::tags::ensure(
      &state.db.conn,
      &user.tenant_id,
      scope_type,
      &scope_id,
      &trimmed,
    )
    .await
    {
      let _ = repo::tags::attach(
        &state.db.conn,
        &user.tenant_id,
        &tag_id,
        &content_hash,
      )
      .await;
    }
  }
  Redirect::to("/app/home").into_response()
}

/// `POST /app/books/{hash}/tags/remove` — remove a tag from a document.
pub(crate) async fn book_tag_remove_post(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(content_hash): Path<String>,
  Form(form): Form<HashMap<String, String>>,
) -> Response {
  let (user, scope_type, scope_id) =
    match tag_context(&state, &headers, &form, &content_hash).await {
      Ok(value) => value,
      Err(response) => return response,
    };
  let name = field(&form, "tag");
  if !name.trim().is_empty() {
    let _ = repo::tags::detach_by_name(
      &state.db.conn,
      &user.tenant_id,
      scope_type,
      &scope_id,
      name.trim(),
      &content_hash,
    )
    .await;
  }
  Redirect::to("/app/home").into_response()
}

/// Shared guard: a logged-in workspace user with valid CSRF and read access to
/// the document, plus the tag scope (`user`/`org`) the tag should live in.
async fn tag_context(
  state: &AppState,
  headers: &HeaderMap,
  form: &HashMap<String, String>,
  content_hash: &str,
) -> Result<(WebUser, &'static str, String), Response> {
  let user = require_workspace_user_response(state, headers).await?;
  if !csrf_ok(&user, form) {
    return Err(error_page(StatusCode::FORBIDDEN, "Invalid CSRF token"));
  }
  let Some(meta) =
    repo::books::access_meta(&state.db.conn, &user.tenant_id, content_hash)
      .await
      .ok()
      .flatten()
  else {
    return Err(error_page(StatusCode::NOT_FOUND, "Document not found"));
  };
  let can_read = repo::access::library(
    &state.db.conn,
    state.entitlements.as_ref(),
    &user.tenant_id,
    &user.user_id,
    user.is_admin(),
    user.personal_sync,
    None,
    &meta.owner_user_id,
    meta.organization_id.as_deref(),
    meta.directory_id.as_deref(),
    content_hash,
  )
  .await
  .map(|access| access.can_read())
  .unwrap_or(false);
  if !can_read {
    return Err(error_page(StatusCode::FORBIDDEN, "No access to document"));
  }
  let (scope_type, scope_id) = match meta.organization_id {
    Some(org) => ("org", org),
    None => ("user", user.user_id.clone()),
  };
  Ok((user, scope_type, scope_id))
}
