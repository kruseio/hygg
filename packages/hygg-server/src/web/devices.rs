use super::*;

pub(crate) async fn devices_page(
  State(state): State<AppState>,
  headers: HeaderMap,
) -> Response {
  let user = match require_workspace_user_response(&state, &headers).await {
    Ok(user) => user,
    Err(response) => return response,
  };
  let devices = repo::devices::list_for_user(
    &state.db.conn,
    &user.tenant_id,
    &user.user_id,
  )
  .await
  .unwrap_or_default();
  // Revoked devices are hidden and don't count: revoking frees a slot.
  let devices: Vec<_> =
    devices.into_iter().filter(|d| d.revoked == 0).collect();
  let active = devices.len() as i64;
  // The web extension may replace the panel head; the core default is a plain
  // count.
  let quota_head =
    state.web_ext.devices_panel_head(&user, active).await.unwrap_or_else(
      || {
        format!(
          r#"<div class="panel-head"><h2>Your devices</h2><span class="device-quota">{active} devices</span></div>"#
        )
      },
    );
  // Disable token creation when the same decision the registration endpoint
  // enforces would refuse it. The core only greys the button out — explaining
  // the refusal is the extension's to do, in its own words.
  let create_disabled = match device_registration_denial(&state, &user).await {
    None => String::new(),
    Some(denial) => {
      format!("disabled {}", state.web_ext.device_create_denied_attrs(&denial))
    }
  };
  let mut rows = String::new();
  for device in devices {
    rows.push_str(&format!(
      r#"<tr><td>{}</td><td>{}</td><td>{}</td>
      <td><a href="/app/devices/{}/permissions">Permissions</a>
      <form method="post" action="/app/devices/{}/revoke">{}<button class="danger" type="submit">Revoke</button></form></td></tr>"#,
      esc(&device.name),
      esc(&device.platform),
      access_label(&device.default_access),
      esc(&device.id),
      esc(&device.id),
      csrf_input(&user)
    ));
  }
  if rows.is_empty() {
    rows.push_str("<tr><td colspan=\"4\">No devices yet.</td></tr>");
  }
  let server_url = request_base_url(&headers);
  let quick_start = cli_quick_start_panel(&server_url, Some(&user.email), None);
  page(
    "Devices",
    Some(&user),
    format!(
      r#"<div class="split-grid">
        <section class="panel"><h2>Create device token</h2>
          <form method="post" action="/app/devices" class="stack">
            {}
            <input name="name" placeholder="Device name" required>
            {}
            <button type="submit" {create_disabled}>Create token</button>
          </form>
        </section>
        {quick_start}
      </div>
      <section class="panel">{quota_head}
        <table><thead><tr><th>Name</th><th>Platform</th><th>Default access</th><th></th></tr></thead>
        <tbody>{rows}</tbody></table>
      </section>"#,
      csrf_input(&user),
      access_select("default_access", "read_write", false, None)
    ),
  )
}

pub(crate) async fn device_create_post(
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
  let server_url = request_base_url(&headers);
  create_device_for_user(
    &state,
    &user,
    &user.user_id,
    &form,
    &server_url,
    "/app/devices",
  )
  .await
}

pub(crate) async fn device_permissions_page(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(device_id): Path<String>,
) -> Response {
  let user = match require_workspace_user_response(&state, &headers).await {
    Ok(user) => user,
    Err(response) => return response,
  };
  let Some(device) =
    repo::devices::find_by_id(&state.db.conn, &user.tenant_id, &device_id)
      .await
      .ok()
      .flatten()
  else {
    return error_page(StatusCode::NOT_FOUND, "Device not found");
  };
  if device.user_id != user.user_id {
    return error_page(StatusCode::NOT_FOUND, "Device not found");
  }
  device_permissions_response(
    &state,
    &user,
    &device,
    &format!("/app/devices/{device_id}/permissions"),
    "Device permissions",
  )
  .await
}

pub(crate) async fn device_permissions_post(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(device_id): Path<String>,
  Form(form): Form<HashMap<String, String>>,
) -> Response {
  let user = match require_workspace_user_response(&state, &headers).await {
    Ok(user) => user,
    Err(response) => return response,
  };
  if !csrf_ok(&user, &form) {
    return error_page(StatusCode::FORBIDDEN, "Invalid CSRF token");
  }
  let Some(device) =
    repo::devices::find_by_id(&state.db.conn, &user.tenant_id, &device_id)
      .await
      .ok()
      .flatten()
  else {
    return error_page(StatusCode::NOT_FOUND, "Device not found");
  };
  if device.user_id != user.user_id {
    return error_page(StatusCode::NOT_FOUND, "Device not found");
  }
  save_device_permissions(&state, &user.tenant_id, &device_id, &form).await;
  Redirect::to(&format!("/app/devices/{device_id}/permissions")).into_response()
}

pub(crate) async fn device_revoke_post(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(device_id): Path<String>,
  Form(form): Form<HashMap<String, String>>,
) -> Response {
  let user = match require_workspace_user_response(&state, &headers).await {
    Ok(user) => user,
    Err(response) => return response,
  };
  if csrf_ok(&user, &form) {
    let _ = repo::devices::revoke(
      &state.db.conn,
      &user.tenant_id,
      &user.user_id,
      &device_id,
    )
    .await;
  }
  Redirect::to("/app/devices").into_response()
}

/// The refusal, if any, that another device registration for this user would
/// meet — drives the create-token button's disabled state. `None` admits it.
pub(crate) async fn device_registration_denial(
  state: &AppState,
  user: &WebUser,
) -> Option<Denial> {
  match state
    .entitlements
    .authorize_device_registration(crate::ext::EntCtx {
      tenant_id: &user.tenant_id,
      user_id: &user.user_id,
      is_admin: user.is_admin(),
    })
    .await
  {
    Ok(()) => None,
    // Refused with wording, or refused flatly with none to offer.
    Err(AppError::Denied(denial)) => Some(denial),
    Err(_) => Some(Denial::new(String::new())),
  }
}

pub(crate) fn device_permissions_content(
  user: &WebUser,
  device: &repo::devices::DeviceRow,
  books: &[repo::books::BookRow],
  overrides: &HashMap<String, String>,
  action: &str,
) -> String {
  let mut rows = String::new();
  for book in books {
    let selected = overrides.get(&book.content_hash).map(String::as_str);
    let effective = selected.unwrap_or(&device.default_access);
    rows.push_str(&format!(
      r#"<tr><td>{}</td><td>{}</td><td class="mono">{}</td><td>{}</td><td>{}</td></tr>"#,
      esc(&book.title),
      esc(&book.format),
      esc(&book.content_hash),
      access_label(effective),
      access_select(
        &format!("book_access:{}", book.content_hash),
        selected.unwrap_or(""),
        true,
        Some(&device.default_access)
      )
    ));
  }
  if rows.is_empty() {
    rows.push_str("<tr><td colspan=\"5\">No synced documents yet.</td></tr>");
  }
  format!(
    r#"<section class="panel"><h2>{}</h2><dl>
      <dt>Platform</dt><dd>{}</dd>
      <dt>Default access</dt><dd>{}</dd>
    </dl></section>
    <form method="post" action="{}">
      {}
      <section class="panel action-panel"><div><h2>Default access</h2></div>{}</section>
      <section class="panel"><table>
        <thead><tr><th>Document</th><th>Format</th><th>Document id</th><th>Effective</th><th>Override</th></tr></thead>
        <tbody>{rows}</tbody>
      </table></section>
      <div class="actions"><button type="submit">Save permissions</button></div>
    </form>"#,
    esc(&device.name),
    esc(&device.platform),
    access_label(&device.default_access),
    esc(action),
    csrf_input(user),
    access_select("default_access", &device.default_access, false, None)
  )
}
