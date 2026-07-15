use super::*;

pub(crate) async fn account_page(
  State(state): State<AppState>,
  headers: HeaderMap,
) -> Response {
  let Some(user) = require_user(&state, &headers).await else {
    return Redirect::to("/login").into_response();
  };
  let has_valid_passkey =
    user_has_valid_passkey(&state, &user.tenant_id, &user.user_id).await;
  let passkeys = repo::passkeys::list_for_user(
    &state.db.conn,
    &user.tenant_id,
    &user.user_id,
  )
  .await
  .unwrap_or_default();
  let password_status_radios =
    password_status_radios(user.password_enabled, has_valid_passkey);
  let passkeys_content = account_passkeys_content(&user, &passkeys);
  // Whatever rows the deployment wants alongside the core's own; none by
  // default, and the summary simply closes up.
  let ext_items = state.web_ext.account_rows(&user).await;
  let _ = repo::sessions::delete_expired(&state.db.conn).await;
  let sessions = repo::sessions::list_for_user(
    &state.db.conn,
    &user.tenant_id,
    &user.user_id,
  )
  .await
  .unwrap_or_default();
  // The account page intentionally omits device API tokens; managing those
  // lives under Devices. Admins still see them on the per-user sessions page.
  let sessions_content = sessions_content(
    &user,
    &sessions,
    &[],
    &user.session_id,
    "/account/sessions",
    false,
  );
  let password_status =
    if user.password_enabled { "enabled" } else { "disabled" };
  let password_status_title = if user.password_enabled {
    "Password enabled"
  } else {
    "Password disabled"
  };
  let password_status_class =
    if user.password_enabled { "status-enabled" } else { "status-disabled" };
  page(
    "Settings",
    Some(&user),
    format!(
      r#"<section class="panel account-card">
        <div class="account-card-header">
          <div class="account-avatar-large">{}</div>
          <div>
            <p class="eyebrow">Settings</p>
            <h1>Account</h1>
          </div>
          <span class="status-pill {}">{}</span>
        </div>
        <div class="account-summary">
          <div class="account-summary-item">
            <span class="summary-icon">{}</span>
            <div><span>Email</span><strong>{}</strong></div>
          </div>
          <div class="account-summary-item">
            <span class="summary-icon">{}</span>
            <div><span>Role</span><strong>{}</strong></div>
          </div>
          {ext_items}
          <div class="account-summary-item">
            <span class="summary-icon">{}</span>
            <div><span>Password auth</span><strong>{}</strong></div>
          </div>
        </div>
        <div class="account-security">
          <form method="post" action="/account/password" class="account-security-form">
            {}
            <input type="hidden" name="action" value="password_status">
            <div class="account-form-title">
              {}<div><h2>Password auth</h2><span>{}</span></div>
            </div>
            {}
            <button type="submit">{}<span>Save</span></button>
          </form>
          <form method="post" action="/account/password" class="account-security-form">
            {}
            <input type="hidden" name="action" value="enable">
            <div class="account-form-title">
              {}<div><h2>Set password</h2><span>Minimum 8 characters</span></div>
            </div>
            <div class="account-password-row">
              <input name="password" type="password" placeholder="New password" minlength="8">
              <button class="secondary" type="submit">{}<span>Set</span></button>
            </div>
          </form>
        </div>
      </section>
      {passkeys_content}
      {sessions_content}"#,
      icon("circle-user"),
      password_status_class,
      password_status_title,
      icon("mail"),
      esc(&user.email),
      icon("shield"),
      esc(user.role.as_str()),
      icon("key-round"),
      password_status,
      csrf_input(&user),
      icon("shield"),
      password_status_title,
      password_status_radios,
      icon("save"),
      csrf_input(&user),
      icon("lock-keyhole"),
      icon("key-round")
    ),
  )
}

pub(crate) async fn account_password_post(
  State(state): State<AppState>,
  headers: HeaderMap,
  Form(form): Form<HashMap<String, String>>,
) -> Response {
  let Some(user) = require_user(&state, &headers).await else {
    return Redirect::to("/login").into_response();
  };
  if !csrf_ok(&user, &form) {
    return error_page(StatusCode::FORBIDDEN, "Invalid CSRF token");
  }
  match field(&form, "action").as_str() {
    "password_status" => {
      let enabled = field(&form, "password_enabled") != "disabled";
      if !enabled
        && !user_has_valid_passkey(&state, &user.tenant_id, &user.user_id).await
      {
        return error_page(
          StatusCode::FORBIDDEN,
          "A valid passkey is required before password auth can be disabled",
        );
      }
      let _ = repo::users::set_password_enabled(
        &state.db.conn,
        &user.tenant_id,
        &user.user_id,
        enabled,
      )
      .await;
    }
    "disable" => {
      if !user_has_valid_passkey(&state, &user.tenant_id, &user.user_id).await {
        return error_page(
          StatusCode::FORBIDDEN,
          "A valid passkey is required before password auth can be disabled",
        );
      }
      let _ = repo::users::set_password_enabled(
        &state.db.conn,
        &user.tenant_id,
        &user.user_id,
        false,
      )
      .await;
    }
    "enable" => {
      let password = field(&form, "password");
      if password.len() >= 8
        && let Ok(hash) = hash_password(&password)
      {
        let _ = repo::users::set_password_hash(
          &state.db.conn,
          &user.tenant_id,
          &user.user_id,
          &hash,
        )
        .await;
      }
    }
    _ => {}
  }
  Redirect::to("/account").into_response()
}
