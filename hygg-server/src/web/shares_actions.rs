//! POST handlers for `/app/shares`: create a share, and accept / decline /
//! revoke one. Both directions consult the entitlement hooks, which allow
//! everyone and impose no cap unless an override says otherwise. An override
//! that refuses supplies its own wording, which these handlers relay.

use super::*;

fn ent_ctx<'a>(
  tenant_id: &'a str,
  user_id: &'a str,
  is_admin: bool,
) -> crate::ext::EntCtx<'a> {
  crate::ext::EntCtx { tenant_id, user_id, is_admin }
}

fn share_access(value: &str) -> &'static str {
  if value == "read_write" { "read_write" } else { "read" }
}

/// `POST /app/shares` — share one of your personal documents with another user.
pub(crate) async fn share_create_post(
  State(state): State<AppState>,
  headers: HeaderMap,
  Form(form): Form<HashMap<String, String>>,
) -> Response {
  let user = match require_workspace_user_response(&state, &headers).await {
    Ok(user) => user,
    Err(response) => return response,
  };
  if !csrf_ok(&user, &form) {
    return error_page(StatusCode::FORBIDDEN, "Invalid CSRF token");
  }
  let content_hash = field(&form, "content_hash");
  let email = field(&form, "email").to_lowercase();
  let access = share_access(&field(&form, "access"));
  if content_hash.is_empty() || email.is_empty() {
    return Redirect::to("/app/shares").into_response();
  }
  let pool = &state.db.conn;
  // You may only share a personal document you own (org documents are shared
  // through the organization's permission model, not here).
  let Some(meta) =
    repo::books::access_meta(pool, &user.tenant_id, &content_hash)
      .await
      .ok()
      .flatten()
  else {
    return error_page(StatusCode::NOT_FOUND, "Document not found");
  };
  if meta.owner_user_id != user.user_id || meta.organization_id.is_some() {
    return error_page(
      StatusCode::FORBIDDEN,
      "You can only share your own personal documents",
    );
  }
  // Resolve the recipient by their tenant-unique email ("username").
  let Ok(Some(target)) =
    repo::users::find_by_email(pool, &user.tenant_id, &email).await
  else {
    return error_page(
      StatusCode::NOT_FOUND,
      "No user with that email in this instance",
    );
  };
  if target.id == user.user_id {
    return error_page(
      StatusCode::BAD_REQUEST,
      "You can't share a document with yourself",
    );
  }
  // Both parties must be allowed to participate in sharing. A hook that says
  // no explains itself; the core just relays it.
  if let Err(err) = state
    .entitlements
    .authorize_share_participant(
      ent_ctx(&user.tenant_id, &user.user_id, user.is_admin()),
      crate::ext::ShareSubject::Caller,
    )
    .await
  {
    return denial_page(err);
  }
  let target_admin = Role::parse(&target.role).is_admin();
  if let Err(err) = state
    .entitlements
    .authorize_share_participant(
      ent_ctx(&user.tenant_id, &target.id, target_admin),
      crate::ext::ShareSubject::OtherParty,
    )
    .await
  {
    return denial_page(err);
  }
  // Enforce the sender's outgoing cap — but re-submitting an already-active
  // share is a harmless no-op that must not be blocked by the cap.
  let already = repo::shares::active_share_exists(
    pool,
    &user.tenant_id,
    &content_hash,
    &target.id,
  )
  .await
  .unwrap_or(false);
  if !already
    && let Some(limit) = state
      .entitlements
      .share_limit(ent_ctx(&user.tenant_id, &user.user_id, user.is_admin()))
      .await
  {
    let count =
      repo::shares::outgoing_active_count(pool, &user.tenant_id, &user.user_id)
        .await
        .unwrap_or(0);
    if count >= limit {
      return error_page(
        StatusCode::FORBIDDEN,
        "You've reached your shared-documents limit",
      );
    }
  }
  if let Ok(outcome) = repo::shares::create(
    pool,
    &user.tenant_id,
    &content_hash,
    &user.user_id,
    &target.id,
    access,
  )
  .await
    && outcome != repo::shares::CreateOutcome::AlreadyActive
  {
    let _ = repo::notifications::upsert(
      pool,
      &user.tenant_id,
      &target.id,
      &format!("share:{content_hash}:{}", user.user_id),
      "info",
      "New shared document",
      &format!("{} shared a document with you.", user.email),
    )
    .await;
  }
  Redirect::to("/app/shares").into_response()
}

/// `POST /app/shares/{id}/accept` — accept an incoming share.
pub(crate) async fn share_accept_post(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(id): Path<String>,
  Form(form): Form<HashMap<String, String>>,
) -> Response {
  let user = match require_workspace_user_response(&state, &headers).await {
    Ok(user) => user,
    Err(response) => return response,
  };
  if !csrf_ok(&user, &form) {
    return error_page(StatusCode::FORBIDDEN, "Invalid CSRF token");
  }
  let pool = &state.db.conn;
  let Some(share) =
    repo::shares::find(pool, &user.tenant_id, &id).await.ok().flatten()
  else {
    return error_page(StatusCode::NOT_FOUND, "Share not found");
  };
  if share.to_user_id != user.user_id || share.status != repo::shares::PENDING {
    return Redirect::to("/app/shares").into_response();
  }
  if let Err(err) = state
    .entitlements
    .authorize_share_participant(
      ent_ctx(&user.tenant_id, &user.user_id, user.is_admin()),
      crate::ext::ShareSubject::Caller,
    )
    .await
  {
    return denial_page(err);
  }
  if let Some(limit) = state
    .entitlements
    .share_limit(ent_ctx(&user.tenant_id, &user.user_id, user.is_admin()))
    .await
  {
    let count = repo::shares::incoming_accepted_count(
      pool,
      &user.tenant_id,
      &user.user_id,
    )
    .await
    .unwrap_or(0);
    if count >= limit {
      return error_page(
        StatusCode::FORBIDDEN,
        "You've reached your incoming shared-documents limit",
      );
    }
  }
  let _ = repo::shares::accept(pool, &user.tenant_id, &id, &user.user_id).await;
  Redirect::to("/app/shares").into_response()
}

/// `POST /app/shares/{id}/decline` — decline an incoming share.
pub(crate) async fn share_decline_post(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(id): Path<String>,
  Form(form): Form<HashMap<String, String>>,
) -> Response {
  let user = match require_workspace_user_response(&state, &headers).await {
    Ok(user) => user,
    Err(response) => return response,
  };
  if !csrf_ok(&user, &form) {
    return error_page(StatusCode::FORBIDDEN, "Invalid CSRF token");
  }
  let _ =
    repo::shares::decline(&state.db.conn, &user.tenant_id, &id, &user.user_id)
      .await;
  Redirect::to("/app/shares").into_response()
}

/// `POST /app/books/{content_hash}/unshare` — the recipient removes a document
/// that was shared with them, dropping it from their library.
pub(crate) async fn share_leave_post(
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
  let _ = repo::shares::leave_by_hash(
    &state.db.conn,
    &user.tenant_id,
    &content_hash,
    &user.user_id,
  )
  .await;
  Redirect::to("/app/home").into_response()
}

/// `POST /app/shares/{id}/revoke` — the sender revokes a share they created.
pub(crate) async fn share_revoke_post(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(id): Path<String>,
  Form(form): Form<HashMap<String, String>>,
) -> Response {
  let user = match require_workspace_user_response(&state, &headers).await {
    Ok(user) => user,
    Err(response) => return response,
  };
  if !csrf_ok(&user, &form) {
    return error_page(StatusCode::FORBIDDEN, "Invalid CSRF token");
  }
  let _ =
    repo::shares::revoke(&state.db.conn, &user.tenant_id, &id, &user.user_id)
      .await;
  Redirect::to("/app/shares").into_response()
}
