use super::*;

pub(crate) async fn login_page() -> Response {
  login_identifier_form("", None)
}

pub(crate) async fn login_post(
  State(state): State<AppState>,
  headers: HeaderMap,
  Form(form): Form<HashMap<String, String>>,
) -> Response {
  let password_present = form.contains_key("password");
  let recovery_present = form.contains_key("recovery_token");
  let login_step = field(&form, "login_step");
  if matches!(login_step.as_str(), "password" | "recovery")
    || password_present
    || recovery_present
  {
    return login_password_post(state, headers, form).await;
  }
  login_identifier_post(state, headers, form).await
}

pub(crate) async fn login_identifier_post(
  state: AppState,
  headers: HeaderMap,
  form: HashMap<String, String>,
) -> Response {
  let email = field(&form, "email").to_lowercase();
  if email.is_empty() {
    return login_identifier_form(&email, Some("Email is required"));
  }
  if login_identifier_rate_limited(&state, &headers).await {
    return error_page(
      StatusCode::TOO_MANY_REQUESTS,
      "Too many login attempts. Try again shortly.",
    );
  }
  let Ok(tenant_id) = default_tenant_id(&state).await else {
    return error_page(
      StatusCode::INTERNAL_SERVER_ERROR,
      "Tenant not available",
    );
  };
  let Ok(Some(user)) =
    repo::users::find_by_email(&state.db.conn, &tenant_id, &email).await
  else {
    return login_identifier_form(&email, Some("Could not continue sign-in"));
  };
  if user.disabled != 0 {
    return login_identifier_form(&email, Some("Could not continue sign-in"));
  }

  let has_passkey = user_has_valid_passkey(&state, &tenant_id, &user.id).await;
  login_auth_form(&user, has_passkey, None)
}

pub(crate) async fn login_password_post(
  state: AppState,
  headers: HeaderMap,
  form: HashMap<String, String>,
) -> Response {
  let email = field(&form, "email").to_lowercase();
  let password = field(&form, "password");
  let recovery_token = field(&form, "recovery_token");
  let Ok(tenant_id) = default_tenant_id(&state).await else {
    return error_page(
      StatusCode::INTERNAL_SERVER_ERROR,
      "Tenant not available",
    );
  };
  let Ok(Some(user)) =
    repo::users::find_by_email(&state.db.conn, &tenant_id, &email).await
  else {
    return login_identifier_form(&email, Some("Invalid credentials"));
  };
  if user.disabled != 0 {
    return login_identifier_form(&email, Some("Invalid credentials"));
  }

  let password_ok = recovery_token.is_empty()
    && user.password_enabled != 0
    && user
      .password_hash
      .as_deref()
      .is_some_and(|hash| verify_password(&password, hash));
  let recovery_ok = if password_ok {
    false
  } else if !recovery_token.is_empty() {
    repo::recovery::consume_matching(
      &state.db.conn,
      &tenant_id,
      &user.id,
      &hash_secret(&recovery_token),
    )
    .await
    .unwrap_or(false)
  } else {
    false
  };

  if !password_ok && !recovery_ok {
    let has_passkey =
      user_has_valid_passkey(&state, &tenant_id, &user.id).await;
    return login_auth_form(&user, has_passkey, Some("Invalid credentials"));
  }
  let redirect_to = login_redirect_for(&state, &user).await;
  create_session_response(&state, &headers, &tenant_id, &user.id, redirect_to)
    .await
}

pub(crate) fn login_identifier_form(
  email: &str,
  error: Option<&str>,
) -> Response {
  let error_html = error
    .map(|e| {
      format!(
        r#"<div class="toast-stack"><div class="toast toast-error" role="alert">{}</div></div>"#,
        esc(e)
      )
    })
    .unwrap_or_default();
  page(
    "Log in",
    None,
    format!(
      r#"<section class="panel auth"><h1>Log in</h1>{error_html}
        <p class="muted">Enter your email first. We will use a passkey when one is available.</p>
        <form method="post" action="/login" class="stack" data-login-step="identifier">
          <input type="hidden" name="login_step" value="identifier">
          <input name="email" type="email" autocomplete="username webauthn" placeholder="Email" value="{}" autofocus>
          <button type="submit">Continue</button>
        </form>
      </section>{}"#,
      esc(email),
      login_identifier_script(),
    ),
  )
}

pub(crate) fn login_identifier_script() -> &'static str {
  r#"<script>
(() => {
  const form = document.querySelector("[data-login-step='identifier']");
  if (!form) return;
  const email = form.querySelector("input[name='email']");
  if (!email) return;
  email.addEventListener("keydown", (event) => {
    if (event.key !== "Enter" || event.isComposing) return;
    event.preventDefault();
    if (form.requestSubmit) form.requestSubmit();
    else form.submit();
  });
})();
</script>"#
}

pub(crate) fn login_auth_form(
  user: &repo::users::UserRow,
  has_passkey: bool,
  error: Option<&str>,
) -> Response {
  let error_html = error
    .map(|e| {
      format!(
        r#"<div class="toast-stack"><div class="toast toast-error" role="alert">{}</div></div>"#,
        esc(e)
      )
    })
    .unwrap_or_default();
  let passkey_html = if has_passkey {
    format!(
      r#"<section class="passkey-first" data-passkey-preferred="true">
        <input type="hidden" name="email" value="{}">
        <p class="muted">Passkey is preferred for this account. Your browser should open the native device prompt.</p>
        <button class="secondary" type="button" id="passkey-login" data-autostart="true">Use passkey</button>
        <p class="form-status" id="passkey-login-status" role="status">Waiting for device prompt...</p>
      </section>"#,
      esc(&user.email)
    )
  } else {
    String::new()
  };
  let password_html = if user.password_enabled != 0 {
    let open = if has_passkey && error.is_none() { "" } else { " open" };
    let summary =
      if has_passkey { "Use password instead" } else { "Use password" };
    format!(
      r#"<details class="password-login"{}><summary>{}</summary>
        <form method="post" action="/login" class="stack">
          <input type="hidden" name="login_step" value="password">
          <input type="hidden" name="email" value="{}">
          <input name="password" type="password" autocomplete="current-password" placeholder="Password" autofocus>
          <button type="submit">Log in</button>
        </form>
      </details>"#,
      open,
      esc(summary),
      esc(&user.email)
    )
  } else {
    r#"<p class="muted">Password auth is disabled for this account.</p>"#
      .to_string()
  };
  let recovery_open =
    if user.password_enabled == 0 || error.is_some() { " open" } else { "" };
  let recovery_html = format!(
    r#"<details class="recovery-token-login"{recovery_open}>
        <summary>Use recovery token</summary>
        <form method="post" action="/login" class="stack">
          <input type="hidden" name="login_step" value="recovery">
          <input type="hidden" name="email" value="{}">
          <input name="recovery_token" type="password" autocomplete="one-time-code" placeholder="One-time recovery token">
          <button type="submit">Log in with recovery token</button>
        </form>
        <p class="muted">Admin-issued recovery tokens expire after 30 minutes and are consumed after one use. They do not re-enable the old password.</p>
      </details>"#,
    esc(&user.email)
  );
  page(
    "Log in",
    None,
    format!(
      r#"<section class="panel auth"><h1>Log in</h1>{error_html}
        <p class="muted">Signing in as <strong>{}</strong>.</p>
        {passkey_html}
        {password_html}
        {recovery_html}
        <p><a href="/login">Use a different email</a></p>
      </section>{}"#,
      esc(&user.email),
      if has_passkey { webauthn_script() } else { "" }
    ),
  )
}
