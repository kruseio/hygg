use super::*;

pub(crate) async fn admin_users_page(
  State(state): State<AppState>,
  headers: HeaderMap,
) -> Response {
  let Some(user) = require_admin(&state, &headers).await else {
    return Redirect::to("/login").into_response();
  };
  let users = repo::users::list_for_tenant(&state.db.conn, &user.tenant_id)
    .await
    .unwrap_or_default();
  let mut rows = String::new();
  for row in users {
    rows.push_str(&format!(
      r#"<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td>
      <td><form method="post" action="/app/admin/users/{}/role">{}{}<button type="submit">Save</button></form></td>
      <td><form method="post" action="/app/admin/users/{}/disabled">{}<input type="hidden" name="disabled" value="{}"><button class="danger" type="submit">{}</button></form></td>
      <td><form method="post" action="/app/admin/users/{}/recovery">{}<button type="submit">Recovery token</button></form>
          <a href="/app/admin/users/{}/passkeys">Passkeys</a>
          <a href="/app/admin/users/{}/sessions">Sessions</a></td></tr>"#,
      esc(&row.email),
      esc(&row.display_name),
      esc(role_label(&row.role)),
      yes_no(row.password_enabled),
      yes_no(row.disabled),
      esc(&row.id),
      csrf_input(&user),
      role_select(&row.role),
      esc(&row.id),
      csrf_input(&user),
      if row.disabled == 0 { "1" } else { "0" },
      if row.disabled == 0 { "Disable" } else { "Enable" },
      esc(&row.id),
      csrf_input(&user),
      esc(&row.id),
      esc(&row.id)
    ));
  }
  page(
    "Admin users",
    Some(&user),
    format!(
      r#"<section class="panel"><h2>Create user</h2>
        <form method="post" action="/app/admin/users" class="stack">
          {}
          <input name="email" type="email" placeholder="Email">
          <input name="display_name" placeholder="Display name">
          <input name="password" type="password" placeholder="Initial password">
          {}
          <button type="submit">Create user</button>
        </form>
      </section>
      <section class="panel"><h2>Users</h2>
        <table><thead><tr><th>Email</th><th>Name</th><th>Role</th><th>Password</th><th>Disabled</th><th>Role change</th><th>Status</th><th>Access</th></tr></thead>
        <tbody>{rows}</tbody></table>
      </section>"#,
      csrf_input(&user),
      role_select("user")
    ),
  )
}

pub(crate) async fn admin_user_create_post(
  State(state): State<AppState>,
  headers: HeaderMap,
  Form(form): Form<HashMap<String, String>>,
) -> Response {
  let Some(admin) = require_admin(&state, &headers).await else {
    return Redirect::to("/login").into_response();
  };
  if !csrf_ok(&admin, &form) {
    return error_page(StatusCode::FORBIDDEN, "Invalid CSRF token");
  }
  let email = field(&form, "email").to_lowercase();
  let display_name = field(&form, "display_name");
  let password = field(&form, "password");
  let role = normalized_role(&field(&form, "role"));
  if email.is_empty() {
    return Redirect::to("/app/admin/users").into_response();
  }
  let hash =
    if password.is_empty() { None } else { hash_password(&password).ok() };
  let name = if display_name.is_empty() { &email } else { &display_name };
  let _ = repo::users::insert(
    &state.db.conn,
    &admin.tenant_id,
    &email,
    name,
    hash.as_deref(),
    role,
  )
  .await;
  Redirect::to("/app/admin/users").into_response()
}

pub(crate) async fn admin_user_role_post(
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
  let role = normalized_role(&field(&form, "role"));
  if role != "admin" && is_last_admin(&state, &admin.tenant_id, &user_id).await
  {
    return error_page(StatusCode::FORBIDDEN, "Cannot demote the last admin");
  }
  let _ =
    repo::users::set_role(&state.db.conn, &admin.tenant_id, &user_id, role)
      .await;
  Redirect::to("/app/admin/users").into_response()
}

pub(crate) async fn admin_user_disabled_post(
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
  let disabled = field(&form, "disabled") == "1";
  if disabled && is_last_admin(&state, &admin.tenant_id, &user_id).await {
    return error_page(StatusCode::FORBIDDEN, "Cannot disable the last admin");
  }
  let _ = repo::users::set_disabled(
    &state.db.conn,
    &admin.tenant_id,
    &user_id,
    disabled,
  )
  .await;
  Redirect::to("/app/admin/users").into_response()
}

pub(crate) async fn admin_recovery_post(
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
  let Some(_) =
    repo::users::find_by_id(&state.db.conn, &admin.tenant_id, &user_id)
      .await
      .ok()
      .flatten()
  else {
    return error_page(StatusCode::NOT_FOUND, "User not found");
  };
  let code = random_secret(32);
  let expires_at = now_millis() + RECOVERY_TTL_MS;
  let _ = repo::recovery::insert(
    &state.db.conn,
    &admin.tenant_id,
    &user_id,
    &hash_secret(&code),
    &admin.user_id,
    expires_at,
  )
  .await;
  one_time_secret_page_with_note(
    &admin,
    "Recovery token",
    &code,
    "This token expires in 30 minutes, can be used once, and does not re-enable the user's old password.",
  )
}
