use super::*;

pub(crate) async fn login_identifier_rate_limited(
  state: &AppState,
  headers: &HeaderMap,
) -> bool {
  let now = now_millis();
  let window_start = now.saturating_sub(LOGIN_IDENTIFIER_WINDOW_MS);
  let (ip, _) = session_metadata(headers);
  let key = ip.unwrap_or_else(|| "local".to_string());
  let mut attempts = state.login_identifier_attempts.lock().await;
  attempts.retain(|_, values| {
    values.retain(|attempted_at| *attempted_at >= window_start);
    !values.is_empty()
  });
  let values = attempts.entry(key).or_default();
  if values.len() >= LOGIN_IDENTIFIER_LIMIT {
    return true;
  }
  values.push(now);
  false
}

pub(crate) async fn create_session_response(
  state: &AppState,
  headers: &HeaderMap,
  tenant_id: &str,
  user_id: &str,
  redirect_to: &str,
) -> Response {
  let _ = repo::sessions::delete_expired(&state.db.conn).await;
  let session_id = random_secret(32);
  let csrf_secret = random_secret(32);
  let expires_at = now_millis() + SESSION_TTL_MS;
  let (ip, user_agent) = session_metadata(headers);
  if repo::sessions::insert(
    &state.db.conn,
    &session_id,
    tenant_id,
    user_id,
    &csrf_secret,
    expires_at,
    ip.as_deref(),
    user_agent.as_deref(),
  )
  .await
  .is_err()
  {
    return error_page(
      StatusCode::INTERNAL_SERVER_ERROR,
      "Could not create session",
    );
  }
  (
    [(header::SET_COOKIE, session_cookie(&session_id))],
    Redirect::to(redirect_to),
  )
    .into_response()
}

pub(crate) async fn create_session_json_response(
  state: &AppState,
  headers: &HeaderMap,
  tenant_id: &str,
  user_id: &str,
) -> Response {
  let _ = repo::sessions::delete_expired(&state.db.conn).await;
  let session_id = random_secret(32);
  let csrf_secret = random_secret(32);
  let expires_at = now_millis() + SESSION_TTL_MS;
  let (ip, user_agent) = session_metadata(headers);
  if repo::sessions::insert(
    &state.db.conn,
    &session_id,
    tenant_id,
    user_id,
    &csrf_secret,
    expires_at,
    ip.as_deref(),
    user_agent.as_deref(),
  )
  .await
  .is_err()
  {
    return json_error(
      StatusCode::INTERNAL_SERVER_ERROR,
      "Could not create session",
    );
  }
  let redirect_to =
    match repo::users::find_by_id(&state.db.conn, tenant_id, user_id)
      .await
      .ok()
      .flatten()
    {
      Some(user) => login_redirect_for(state, &user).await,
      None => "/account",
    };
  (
    [(header::SET_COOKIE, session_cookie(&session_id))],
    Json(json!({ "ok": true, "redirectTo": redirect_to })),
  )
    .into_response()
}

/// Mint a web session for an already-authenticated user and return the
/// `Set-Cookie` header value that logs the browser in, or `None` if the
/// session could not be persisted.
///
/// This is the seam an in-process embedder uses to hand a user it has
/// authenticated by its own means — e.g. a one-time ticket redeemed from a
/// bearer-token client — into the server-rendered web UI. The caller is
/// responsible for having established the user's identity; this only issues
/// the session. The cookie's name and format stay defined here so the whole
/// contract lives in one place.
pub async fn issue_session_cookie(
  state: &AppState,
  headers: &HeaderMap,
  tenant_id: &str,
  user_id: &str,
) -> Option<String> {
  let _ = repo::sessions::delete_expired(&state.db.conn).await;
  let session_id = random_secret(32);
  let csrf_secret = random_secret(32);
  let expires_at = now_millis() + SESSION_TTL_MS;
  let (ip, user_agent) = session_metadata(headers);
  repo::sessions::insert(
    &state.db.conn,
    &session_id,
    tenant_id,
    user_id,
    &csrf_secret,
    expires_at,
    ip.as_deref(),
    user_agent.as_deref(),
  )
  .await
  .ok()?;
  Some(session_cookie(&session_id))
}

pub async fn default_tenant_id(state: &AppState) -> Result<String, ()> {
  if let Ok(Some(id)) =
    repo::tenants::find_id_by_slug(&state.db.conn, DEFAULT_TENANT_SLUG).await
  {
    return Ok(id);
  }
  ensure_default_tenant(state).await.map_err(|_| ())
}
