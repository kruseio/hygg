use super::*;

pub(crate) async fn admin_devices_page(
  State(state): State<AppState>,
  headers: HeaderMap,
) -> Response {
  let Some(admin) = require_admin(&state, &headers).await else {
    return Redirect::to("/login").into_response();
  };
  let devices =
    repo::devices::list_for_tenant(&state.db.conn, &admin.tenant_id)
      .await
      .unwrap_or_default();
  let mut rows = String::new();
  for d in devices {
    rows.push_str(&format!(
      r#"<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td>
      <td><a href="/app/admin/devices/{}/permissions">Permissions</a></td>
      <td><form method="post" action="/app/admin/devices/{}/token">{}<button type="submit">New token</button></form></td>
      <td><form method="post" action="/app/admin/devices/{}/revoke">{}<button class="danger" type="submit">Revoke</button></form></td></tr>"#,
      esc(&d.email),
      esc(&d.name),
      esc(&d.platform),
      access_label(&d.default_access),
      yes_no(d.revoked),
      esc(&d.id),
      esc(&d.id),
      csrf_input(&admin),
      esc(&d.id),
      csrf_input(&admin)
    ));
  }
  page(
    "Admin devices",
    Some(&admin),
    format!(
      r#"<section class="panel"><h2>Create device</h2>
        <form method="post" action="/app/admin/devices" class="stack">
          {}
          <input name="user_id" placeholder="User id">
          <input name="name" placeholder="Device name" required>
          <input name="platform" placeholder="Platform">
          {}
          <button type="submit">Create token</button>
        </form>
      </section>
      <section class="panel"><h2>Devices</h2>
        <table><thead><tr><th>User</th><th>Name</th><th>Platform</th><th>Default access</th><th>Revoked</th><th>Permissions</th><th>Token</th><th></th></tr></thead>
        <tbody>{rows}</tbody></table>
      </section>"#,
      csrf_input(&admin),
      access_select("default_access", "read_write", false, None)
    ),
  )
}

pub(crate) async fn admin_device_create_post(
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
  let user_id = field(&form, "user_id");
  let server_url = request_base_url(&headers);
  create_device_for_user(
    &state,
    &admin,
    &user_id,
    &form,
    &server_url,
    "/app/admin/devices",
  )
  .await
}

pub(crate) async fn admin_device_permissions_page(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(device_id): Path<String>,
) -> Response {
  let Some(admin) = require_admin(&state, &headers).await else {
    return Redirect::to("/login").into_response();
  };
  let Some(device) =
    repo::devices::find_by_id(&state.db.conn, &admin.tenant_id, &device_id)
      .await
      .ok()
      .flatten()
  else {
    return error_page(StatusCode::NOT_FOUND, "Device not found");
  };
  device_permissions_response(
    &state,
    &admin,
    &device,
    &format!("/app/admin/devices/{device_id}/permissions"),
    "Admin device permissions",
  )
  .await
}

pub(crate) async fn admin_device_permissions_post(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(device_id): Path<String>,
  Form(form): Form<HashMap<String, String>>,
) -> Response {
  let Some(admin) = require_admin(&state, &headers).await else {
    return Redirect::to("/login").into_response();
  };
  if !csrf_ok(&admin, &form) {
    return error_page(StatusCode::FORBIDDEN, "Invalid CSRF token");
  }
  if repo::devices::find_by_id(&state.db.conn, &admin.tenant_id, &device_id)
    .await
    .ok()
    .flatten()
    .is_none()
  {
    return error_page(StatusCode::NOT_FOUND, "Device not found");
  }
  save_device_permissions(&state, &admin.tenant_id, &device_id, &form).await;
  Redirect::to(&format!("/app/admin/devices/{device_id}/permissions"))
    .into_response()
}

pub(crate) async fn admin_device_revoke_post(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(device_id): Path<String>,
  Form(form): Form<HashMap<String, String>>,
) -> Response {
  let Some(admin) = require_admin(&state, &headers).await else {
    return Redirect::to("/login").into_response();
  };
  if csrf_ok(&admin, &form) {
    let _ =
      repo::devices::revoke_any(&state.db.conn, &admin.tenant_id, &device_id)
        .await;
  }
  Redirect::to("/app/admin/devices").into_response()
}

pub(crate) async fn admin_device_token_post(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(device_id): Path<String>,
  Form(form): Form<HashMap<String, String>>,
) -> Response {
  let Some(admin) = require_admin(&state, &headers).await else {
    return Redirect::to("/login").into_response();
  };
  if !csrf_ok(&admin, &form) {
    return error_page(StatusCode::FORBIDDEN, "Invalid CSRF token");
  }
  let Some(device) =
    repo::devices::find_by_id(&state.db.conn, &admin.tenant_id, &device_id)
      .await
      .ok()
      .flatten()
  else {
    return error_page(StatusCode::NOT_FOUND, "Device not found");
  };
  let token = generate_token();
  let _ = repo::tokens::insert(
    &state.db.conn,
    &admin.tenant_id,
    &device.id,
    &token.prefix,
    &token.hash,
  )
  .await;
  // Prefill the auth line with the device owner's email (the CLI's username),
  // not the admin's.
  let username =
    repo::users::find_by_id(&state.db.conn, &admin.tenant_id, &device.user_id)
      .await
      .ok()
      .flatten()
      .map(|u| u.email)
      .unwrap_or_default();
  let server_url = request_base_url(&headers);
  device_token_page(
    &admin,
    &username,
    &token.full,
    &server_url,
    "/app/admin/devices",
  )
}
