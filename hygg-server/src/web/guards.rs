use super::*;

#[derive(Clone, Debug)]
pub struct WebUser {
  pub session_id: String,
  pub tenant_id: String,
  pub user_id: String,
  pub email: String,
  pub display_name: String,
  pub role: Role,
  /// Whether the user may sync/manage their personal library, as resolved by
  /// the injected hook. True unless an override says otherwise.
  pub personal_sync: bool,
  /// Whether the workspace UI (home / devices / organizations) is available —
  /// also from the hook, and likewise true unless overridden.
  pub workspace: bool,
  pub password_enabled: bool,
  pub csrf_secret: String,
  /// Undismissed notifications for the bell, loaded with the session.
  pub notifications: Vec<repo::notifications::NotificationRow>,
  /// Extra sidenav links for the Admin group, pre-rendered from the web
  /// extension's [`NavLink`]s (empty for non-admins and on self-host).
  ///
  /// [`NavLink`]: crate::ext::NavLink
  pub nav_admin_extra: String,
}

impl WebUser {
  pub fn is_admin(&self) -> bool {
    self.role.is_admin()
  }

  pub fn has_workspace_access(&self) -> bool {
    self.workspace
  }
}

pub async fn current_user(
  state: &AppState,
  headers: &HeaderMap,
) -> Option<WebUser> {
  let session_id = cookie_value(headers, SESSION_COOKIE)?;
  let row = repo::sessions::find(&state.db.conn, &session_id).await.ok()??;
  if row.expires_at <= now_millis() || row.disabled != 0 {
    let _ = repo::sessions::delete(&state.db.conn, &session_id).await;
    return None;
  }
  let _ = repo::sessions::touch(
    &state.db.conn,
    &session_id,
    now_millis() + SESSION_TTL_MS,
  )
  .await;
  let notifications = repo::notifications::list_undismissed(
    &state.db.conn,
    &row.tenant_id,
    &row.user_id,
  )
  .await
  .unwrap_or_default();
  let role = Role::parse(&row.role);
  let decision = state
    .entitlements
    .resolve(crate::ext::EntCtx {
      tenant_id: &row.tenant_id,
      user_id: &row.user_id,
      is_admin: role.is_admin(),
    })
    .await;
  let nav_admin_extra = if role.is_admin() {
    state
      .web_ext
      .admin_nav_links()
      .iter()
      .map(|link| nav_item(link.href, link.label, link.icon))
      .collect()
  } else {
    String::new()
  };
  Some(WebUser {
    session_id: row.session_id,
    tenant_id: row.tenant_id,
    user_id: row.user_id,
    email: row.email,
    display_name: row.display_name,
    role,
    personal_sync: role.is_admin() || decision.personal_sync,
    workspace: decision.workspace,
    password_enabled: row.password_enabled != 0,
    csrf_secret: row.csrf_secret,
    notifications,
    nav_admin_extra,
  })
}

pub async fn require_user(
  state: &AppState,
  headers: &HeaderMap,
) -> Option<WebUser> {
  current_user(state, headers).await
}

pub(crate) async fn require_workspace_user_response(
  state: &AppState,
  headers: &HeaderMap,
) -> Result<WebUser, Response> {
  let Some(user) = current_user(state, headers).await else {
    return Err(Redirect::to("/login").into_response());
  };
  if !user.has_workspace_access() {
    if user.is_admin() {
      return Err(Redirect::to("/app/admin/dashboard").into_response());
    }
    return Err(
      Redirect::to(state.web_ext.no_workspace_redirect()).into_response(),
    );
  }
  Ok(user)
}

pub async fn require_admin_response(
  state: &AppState,
  headers: &HeaderMap,
) -> Result<WebUser, Response> {
  let Some(user) = current_user(state, headers).await else {
    return Err(Redirect::to("/login").into_response());
  };
  if !user.is_admin() {
    return Err(error_page(StatusCode::FORBIDDEN, "Admin required"));
  }
  Ok(user)
}

/// Allow a tenant admin or an owner of the given organization to manage it.
/// Returns the loaded org row so the caller need not re-fetch it.
pub async fn require_org_manager(
  state: &AppState,
  headers: &HeaderMap,
  organization_id: &str,
) -> Result<(WebUser, repo::organizations::OrganizationRow), Response> {
  let Some(user) = current_user(state, headers).await else {
    return Err(Redirect::to("/login").into_response());
  };
  let Some(org) = repo::organizations::find_by_id(
    &state.db.conn,
    &user.tenant_id,
    organization_id,
  )
  .await
  .ok()
  .flatten() else {
    return Err(error_page(StatusCode::NOT_FOUND, "Organization not found"));
  };
  if user.is_admin() {
    return Ok((user, org));
  }
  let role = repo::organizations::user_role(
    &state.db.conn,
    &user.tenant_id,
    organization_id,
    &user.user_id,
  )
  .await
  .ok()
  .flatten();
  if role.as_deref() == Some("owner") {
    Ok((user, org))
  } else {
    Err(error_page(StatusCode::FORBIDDEN, "Organization owner required"))
  }
}

pub(crate) async fn require_admin(
  state: &AppState,
  headers: &HeaderMap,
) -> Option<WebUser> {
  let user = current_user(state, headers).await?;
  user.is_admin().then_some(user)
}

/// Where a user lands right after logging in: admins on the dashboard, users
/// with workspace access (per the entitlements hook) on the home page, and
/// anyone else on their account page.
pub(crate) async fn login_redirect_for(
  state: &AppState,
  user: &repo::users::UserRow,
) -> &'static str {
  let role = Role::parse(&user.role);
  if role.is_admin() {
    return "/app/admin/dashboard";
  }
  let decision = state
    .entitlements
    .resolve(crate::ext::EntCtx {
      tenant_id: &user.tenant_id,
      user_id: &user.id,
      is_admin: false,
    })
    .await;
  if decision.workspace { "/app/home" } else { "/account" }
}

pub(crate) async fn is_last_admin(
  state: &AppState,
  tenant_id: &str,
  user_id: &str,
) -> bool {
  let Ok(Some(user)) =
    repo::users::find_by_id(&state.db.conn, tenant_id, user_id).await
  else {
    return false;
  };
  if user.role != "admin" {
    return false;
  }
  repo::users::count_admins(&state.db.conn, tenant_id).await.unwrap_or(0) <= 1
}

pub(crate) async fn load_active_passkeys(
  state: &AppState,
  tenant_id: &str,
  user_id: &str,
) -> Vec<(repo::passkeys::PasskeyRow, Passkey)> {
  repo::passkeys::list_active_for_user(&state.db.conn, tenant_id, user_id)
    .await
    .unwrap_or_default()
    .into_iter()
    .filter_map(|row| {
      let passkey = serde_json::from_str::<Passkey>(&row.passkey_json).ok()?;
      Some((row, passkey))
    })
    .collect()
}

pub(crate) async fn user_has_valid_passkey(
  state: &AppState,
  tenant_id: &str,
  user_id: &str,
) -> bool {
  !load_active_passkeys(state, tenant_id, user_id).await.is_empty()
}

pub(crate) fn credential_id_text(passkey: &Passkey) -> String {
  URL_SAFE_NO_PAD.encode(passkey.cred_id().as_ref())
}

pub(crate) fn passkey_user_uuid(tenant_id: &str, user_id: &str) -> Uuid {
  let mut hasher = Sha256::new();
  hasher.update(tenant_id.as_bytes());
  hasher.update(b":");
  hasher.update(user_id.as_bytes());
  let digest = hasher.finalize();
  let mut bytes = [0_u8; 16];
  bytes.copy_from_slice(&digest[..16]);
  bytes[6] = (bytes[6] & 0x0f) | 0x50;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  Uuid::from_bytes(bytes)
}
