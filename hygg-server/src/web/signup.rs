use super::*;

pub(crate) async fn signup_page() -> Response {
  auth_page(
    "Sign up",
    "/signup",
    "Create account",
    None,
    "",
    true,
    false,
    AuthFormValues::default(),
  )
}

pub(crate) async fn signup_post(
  State(state): State<AppState>,
  headers: HeaderMap,
  Form(form): Form<HashMap<String, String>>,
) -> Response {
  let email = field(&form, "email").to_lowercase();
  let password = field(&form, "password");
  let display_name = field(&form, "display_name");
  if email.is_empty() || password_complexity_error(&password).is_some() {
    return auth_page(
      "Sign up",
      "/signup",
      "Create account",
      Some(password_complexity_error(&password).unwrap_or("Email is required")),
      "",
      true,
      false,
      AuthFormValues {
        email: &email,
        display_name: &display_name,
        password: &password,
      },
    );
  }
  let Ok(tenant_id) = default_tenant_id(&state).await else {
    return error_page(
      StatusCode::INTERNAL_SERVER_ERROR,
      "Tenant not available",
    );
  };
  let Ok(hash) = hash_password(&password) else {
    return error_page(
      StatusCode::INTERNAL_SERVER_ERROR,
      "Could not hash password",
    );
  };
  let name = if display_name.is_empty() { &email } else { &display_name };
  let inserted = repo::users::insert(
    &state.db.conn,
    &tenant_id,
    &email,
    name,
    Some(&hash),
    "user",
  )
  .await;
  let Ok(user_id) = inserted else {
    return auth_page(
      "Sign up",
      "/signup",
      "Create account",
      Some("Account already exists or cannot be created"),
      "",
      true,
      false,
      AuthFormValues {
        email: &email,
        display_name: &display_name,
        password: &password,
      },
    );
  };
  create_session_response(&state, &headers, &tenant_id, &user_id, "/account")
    .await
}

pub(crate) async fn logout_post(
  State(state): State<AppState>,
  headers: HeaderMap,
  Form(form): Form<HashMap<String, String>>,
) -> Response {
  if let Some(user) = current_user(&state, &headers).await
    && csrf_ok(&user, &form)
  {
    let _ = repo::sessions::delete(&state.db.conn, &user.session_id).await;
  }
  ([(header::SET_COOKIE, delete_cookie())], Redirect::to("/login"))
    .into_response()
}
