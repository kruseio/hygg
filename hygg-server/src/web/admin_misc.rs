use super::*;

pub(crate) async fn admin_passkeys_page(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(user_id): Path<String>,
) -> Response {
  let Some(admin) = require_admin(&state, &headers).await else {
    return Redirect::to("/login").into_response();
  };
  let passkeys =
    repo::passkeys::list_for_user(&state.db.conn, &admin.tenant_id, &user_id)
      .await
      .unwrap_or_default();
  page(
    "Admin passkeys",
    Some(&admin),
    passkey_table(&passkeys, Some((&admin, &user_id))),
  )
}

pub(crate) async fn admin_passkey_revoke_post(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path((user_id, passkey_id)): Path<(String, String)>,
  Form(form): Form<HashMap<String, String>>,
) -> Response {
  let Some(admin) = require_admin(&state, &headers).await else {
    return Redirect::to("/login").into_response();
  };
  if csrf_ok(&admin, &form) {
    let _ = repo::passkeys::revoke(
      &state.db.conn,
      &admin.tenant_id,
      &user_id,
      &passkey_id,
    )
    .await;
  }
  Redirect::to(&format!("/app/admin/users/{user_id}/passkeys")).into_response()
}

pub(crate) async fn admin_sessions_page(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(user_id): Path<String>,
) -> Response {
  let Some(admin) = require_admin(&state, &headers).await else {
    return Redirect::to("/login").into_response();
  };
  let Some(target) =
    repo::users::find_by_id(&state.db.conn, &admin.tenant_id, &user_id)
      .await
      .ok()
      .flatten()
  else {
    return error_page(StatusCode::NOT_FOUND, "User not found");
  };
  let _ = repo::sessions::delete_expired(&state.db.conn).await;
  let sessions =
    repo::sessions::list_for_user(&state.db.conn, &admin.tenant_id, &user_id)
      .await
      .unwrap_or_default();
  let token_sessions =
    repo::tokens::list_for_user(&state.db.conn, &admin.tenant_id, &user_id)
      .await
      .unwrap_or_default();
  let current_id =
    if target.id == admin.user_id { admin.session_id.as_str() } else { "" };
  page(
    "Admin sessions",
    Some(&admin),
    format!(
      r#"<section class="panel"><h2>{}</h2><dl>
        <dt>Email</dt><dd>{}</dd>
        <dt>Role</dt><dd>{}</dd>
      </dl></section>{}"#,
      esc(&target.display_name),
      esc(&target.email),
      esc(role_label(&target.role)),
      sessions_content(
        &admin,
        &sessions,
        &token_sessions,
        current_id,
        &format!("/app/admin/users/{user_id}/sessions"),
        true
      )
    ),
  )
}

pub(crate) async fn admin_session_revoke_post(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path((user_id, session_id)): Path<(String, String)>,
  Form(form): Form<HashMap<String, String>>,
) -> Response {
  let Some(admin) = require_admin(&state, &headers).await else {
    return Redirect::to("/login").into_response();
  };
  if !csrf_ok(&admin, &form) {
    return error_page(StatusCode::FORBIDDEN, "Invalid CSRF token");
  }
  let revoking_current =
    user_id == admin.user_id && session_id == admin.session_id;
  let _ = repo::sessions::revoke_for_user(
    &state.db.conn,
    &admin.tenant_id,
    &user_id,
    &session_id,
  )
  .await;
  if revoking_current {
    return ([(header::SET_COOKIE, delete_cookie())], Redirect::to("/login"))
      .into_response();
  }
  Redirect::to(&format!("/app/admin/users/{user_id}/sessions")).into_response()
}

pub(crate) async fn admin_sessions_revoke_all_post(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(user_id): Path<String>,
  Form(form): Form<HashMap<String, String>>,
) -> Response {
  let Some(admin) = require_admin(&state, &headers).await else {
    return Redirect::to("/login").into_response();
  };
  if !csrf_ok(&admin, &form) {
    return error_page(StatusCode::FORBIDDEN, "Invalid CSRF token");
  }
  let revoking_current = user_id == admin.user_id;
  let _ = repo::sessions::revoke_all_for_user(
    &state.db.conn,
    &admin.tenant_id,
    &user_id,
  )
  .await;
  if revoking_current {
    return ([(header::SET_COOKIE, delete_cookie())], Redirect::to("/login"))
      .into_response();
  }
  Redirect::to(&format!("/app/admin/users/{user_id}/sessions")).into_response()
}
