//! Event-driven limit notifications. Called after usage increases (a member is
//! added, a document is uploaded, a device is registered) to raise a warning
//! (>=80%) or critical (>=100%) notification to an org's owners + tenant
//! admins, and a server-storage warning to admins. Idempotent per condition.

use super::*;

const WARN_PCT: i64 = 80;

/// `POST /app/notifications/{id}/dismiss` — dismiss one of the caller's
/// notifications, then return to the page they were on.
pub(crate) async fn notification_dismiss_post(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(id): Path<String>,
  Form(form): Form<HashMap<String, String>>,
) -> Response {
  let Some(user) = current_user(&state, &headers).await else {
    return Redirect::to("/login").into_response();
  };
  if csrf_ok(&user, &form) {
    let _ = repo::notifications::dismiss(
      &state.db.conn,
      &user.tenant_id,
      &user.user_id,
      &id,
    )
    .await;
  }
  // Same-origin path of the referring page (open-redirect safe), else home.
  let back = headers
    .get(header::REFERER)
    .and_then(|value| value.to_str().ok())
    .and_then(|referer| referer.parse::<axum::http::Uri>().ok())
    .map(|uri| uri.path().to_string())
    .filter(|path| path.starts_with("/app"))
    .unwrap_or_else(|| "/app/home".to_string());
  Redirect::to(&back).into_response()
}

/// Re-evaluate an organization's seat/storage/device limits and notify.
pub(crate) async fn check_org(
  state: &AppState,
  tenant_id: &str,
  organization_id: &str,
) {
  let Some(org) =
    repo::organizations::find_by_id(&state.db.conn, tenant_id, organization_id)
      .await
      .ok()
      .flatten()
  else {
    return;
  };
  // The budgets come from the entitlements hook, which reports everything
  // unlimited unless overridden (`emit` skips a limit of 0), so by default
  // nothing here can ever fire.
  let limits = state
    .entitlements
    .org_limits(crate::ext::OrgCtx { tenant_id, organization_id })
    .await;
  let seat_limit = limits.seats.unwrap_or(0);
  let storage_limit = limits.storage_bytes.unwrap_or(0);
  let device_limit = limits.devices.unwrap_or(0);
  if seat_limit <= 0 && storage_limit <= 0 && device_limit <= 0 {
    return;
  }
  let usage = org_usage(state, tenant_id, organization_id).await;
  let recipients = org_recipients(state, tenant_id, organization_id).await;
  if recipients.is_empty() {
    return;
  }
  let prefix = |kind: &str| format!("org:{organization_id}:{kind}");
  emit(
    state,
    tenant_id,
    &recipients,
    &prefix("seats"),
    &org.name,
    Resource {
      label: "seats",
      used: usage.seats,
      limit: seat_limit,
      bytes: false,
    },
  )
  .await;
  emit(
    state,
    tenant_id,
    &recipients,
    &prefix("storage"),
    &org.name,
    Resource {
      label: "storage",
      used: usage.storage,
      limit: storage_limit,
      bytes: true,
    },
  )
  .await;
  emit(
    state,
    tenant_id,
    &recipients,
    &prefix("devices"),
    &org.name,
    Resource {
      label: "devices",
      used: usage.devices,
      limit: device_limit,
      bytes: false,
    },
  )
  .await;
}

/// Re-evaluate every organization the user belongs to (after they add a
/// device).
pub(crate) async fn check_user_orgs(
  state: &AppState,
  tenant_id: &str,
  user_id: &str,
) {
  let orgs =
    repo::organizations::list_for_user(&state.db.conn, tenant_id, user_id)
      .await
      .unwrap_or_default();
  for org in orgs {
    check_org(state, tenant_id, &org.id).await;
  }
}

/// Warn admins as total server storage approaches/exceeds the configured
/// budget.
pub(crate) async fn check_server_storage(state: &AppState, tenant_id: &str) {
  let Some(limit) = state.config.server_storage_limit_bytes else {
    return;
  };
  let used =
    repo::books::total_storage(&state.db.conn, tenant_id).await.unwrap_or(0);
  let recipients =
    repo::users::admin_ids(&state.db.conn, tenant_id).await.unwrap_or_default();
  emit(
    state,
    tenant_id,
    &recipients,
    "server:storage",
    "Server",
    Resource { label: "storage", used, limit, bytes: true },
  )
  .await;
}

struct Resource {
  label: &'static str,
  used: i64,
  limit: i64,
  bytes: bool,
}

async fn emit(
  state: &AppState,
  tenant_id: &str,
  recipients: &[String],
  key_prefix: &str,
  name: &str,
  resource: Resource,
) {
  if resource.limit <= 0 {
    return;
  }
  let pct = percent(resource.used, resource.limit);
  let fmt = |value: i64| {
    if resource.bytes { format_bytes(value) } else { value.to_string() }
  };
  let (suffix, severity, title, body) = if resource.used >= resource.limit {
    (
      "critical",
      "critical",
      format!("{name}: {} limit reached", resource.label),
      format!(
        "{} of {} {} used — new additions are blocked.",
        fmt(resource.used),
        fmt(resource.limit),
        resource.label,
      ),
    )
  } else if pct >= WARN_PCT {
    (
      "warning",
      "warning",
      format!("{name}: {} almost full", resource.label),
      format!(
        "{} of {} {} used ({pct}%).",
        fmt(resource.used),
        fmt(resource.limit),
        resource.label,
      ),
    )
  } else {
    return;
  };
  let key = format!("{key_prefix}:{suffix}");
  for user_id in recipients {
    let _ = repo::notifications::upsert(
      &state.db.conn,
      tenant_id,
      user_id,
      &key,
      severity,
      &title,
      &body,
    )
    .await;
  }
}

/// An org's notification recipients: its owners plus the tenant's admins.
async fn org_recipients(
  state: &AppState,
  tenant_id: &str,
  organization_id: &str,
) -> Vec<String> {
  let mut ids: Vec<String> = repo::organizations::list_members(
    &state.db.conn,
    tenant_id,
    organization_id,
  )
  .await
  .unwrap_or_default()
  .into_iter()
  .filter(|m| m.role == "owner")
  .map(|m| m.user_id)
  .collect();
  ids.extend(
    repo::users::admin_ids(&state.db.conn, tenant_id).await.unwrap_or_default(),
  );
  ids.sort();
  ids.dedup();
  ids
}
