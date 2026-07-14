use hygg_shared::sync::SyncMode;

use super::*;

/// Set the account-wide sync ceiling for a document (`full` | `metadata` |
/// `off`). Owner-only, mirroring the other per-document controls. Each of the
/// owner's devices then clamps its local preference no higher than this.
pub(crate) async fn book_sync_mode_post(
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
  if owned_book_id(&state, &user, &content_hash).await.is_none() {
    return error_page(StatusCode::NOT_FOUND, "Document not found");
  }
  let mode = SyncMode::from_token_or_default(
    form.get("sync_mode").map(String::as_str).unwrap_or("full"),
  );
  let _ = repo::books::set_sync_mode(
    &state.db.conn,
    &user.tenant_id,
    &content_hash,
    mode,
  )
  .await;
  Redirect::to("/app/home").into_response()
}

/// Delete a document's stored bytes from the server while keeping its metadata
/// row (and the reader's local copy). Owner-only.
pub(crate) async fn book_blob_delete_post(
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
  let Some(book_id) = owned_book_id(&state, &user, &content_hash).await else {
    return error_page(StatusCode::NOT_FOUND, "Document not found");
  };
  let _ =
    repo::blobs::delete_for_book(&state.db.conn, &user.tenant_id, &book_id)
      .await;
  Redirect::to("/app/home").into_response()
}

/// Delete a document entirely: its stored bytes and its metadata row.
/// Owner-only.
pub(crate) async fn book_delete_post(
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
  let Some(book_id) = owned_book_id(&state, &user, &content_hash).await else {
    return error_page(StatusCode::NOT_FOUND, "Document not found");
  };
  // Drop the blob first: FK cascade is not enabled, so removing the metadata
  // row alone would orphan the stored bytes.
  let _ =
    repo::blobs::delete_for_book(&state.db.conn, &user.tenant_id, &book_id)
      .await;
  let removed = repo::books::delete_for_owner(
    &state.db.conn,
    &user.tenant_id,
    &user.user_id,
    &content_hash,
  )
  .await
  .unwrap_or(false);
  if !removed {
    return error_page(StatusCode::NOT_FOUND, "Document not found");
  }
  // Drop any shares of this document so recipients lose access with the owner's
  // copy (the shares table keys on content hash, not a book-id FK).
  let _ = repo::shares::delete_for_hash(
    &state.db.conn,
    &user.tenant_id,
    &content_hash,
  )
  .await;
  Redirect::to("/app/home").into_response()
}

/// The book id for a content hash the caller owns, or `None` when it does not
/// exist or belongs to someone else (deletes are owner-only).
pub(crate) async fn owned_book_id(
  state: &AppState,
  user: &WebUser,
  content_hash: &str,
) -> Option<String> {
  if !repo::books::user_owns_hash(
    &state.db.conn,
    &user.tenant_id,
    &user.user_id,
    content_hash,
  )
  .await
  .unwrap_or(false)
  {
    return None;
  }
  repo::books::find_id_by_hash(&state.db.conn, &user.tenant_id, content_hash)
    .await
    .ok()
    .flatten()
}
