//! Bearer-token authentication. Implements `FromRequestParts` for `Principal`
//! so any handler can take a `Principal` argument to require (and identify) an
//! authenticated device. The credential is three parts, all required:
//!
//! 1. the `Authorization: Bearer prefix.secret` token (looked up + verified),
//! 2. an `X-Hygg-User` header that must match the device owner's email (a
//!    leaked token is not usable without also knowing the username), and
//! 3. an `X-Hygg-Machine-Id` header bound to the device on first use — a later
//!    request from a different machine is rejected, so one token can't be
//!    copied to and used from several machines.
//!
//! Failed attempts are rate-limited per client IP to blunt credential spraying.

use axum::extract::FromRequestParts;
use axum::http::HeaderMap;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use hygg_shared::sync::headers::{MACHINE_ID_HEADER, USER_HEADER};

use crate::auth::token::{split_token, verify_secret};
use crate::auth::{AccessLevel, Principal, Role};
use crate::error::AppError;
use crate::repo;
use crate::state::AppState;
use crate::util::now_millis;

/// Sliding window over which failed auth attempts are counted.
const AUTH_FAILURE_WINDOW_MS: i64 = 60_000;
/// Failed auth attempts allowed per client IP within the window before further
/// attempts are rejected with 429 (until the window drains).
const AUTH_FAILURE_LIMIT: usize = 10;

impl FromRequestParts<AppState> for Principal {
  type Rejection = AppError;

  async fn from_request_parts(
    parts: &mut Parts,
    state: &AppState,
  ) -> Result<Self, Self::Rejection> {
    let token = bearer_token(&parts.headers).ok_or(AppError::Unauthorized)?;
    let username = header_str(&parts.headers, USER_HEADER);
    let machine_id = header_str(&parts.headers, MACHINE_ID_HEADER);
    let ip = client_ip(&parts.headers);
    authenticate(state, &token, username.as_deref(), machine_id.as_deref(), &ip)
      .await
  }
}

/// The `Authorization: Bearer …` token, if present. Exposed for the SSE
/// `events` handler, which resolves credentials from headers *or* query params.
pub(crate) fn bearer_token(headers: &HeaderMap) -> Option<String> {
  let value = headers.get(AUTHORIZATION)?.to_str().ok()?;
  value.strip_prefix("Bearer ").map(str::to_string)
}

/// A trimmed, non-empty header value, or `None`.
pub(crate) fn header_str(headers: &HeaderMap, name: &str) -> Option<String> {
  headers
    .get(name)
    .and_then(|value| value.to_str().ok())
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .map(str::to_string)
}

/// Best-effort client IP for rate limiting: the first `X-Forwarded-For` hop (or
/// `X-Real-IP`) when behind a proxy, else a shared `"local"` bucket.
pub(crate) fn client_ip(headers: &HeaderMap) -> String {
  headers
    .get("x-forwarded-for")
    .and_then(|value| value.to_str().ok())
    .and_then(|value| value.split(',').next())
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .or_else(|| {
      headers
        .get("x-real-ip")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    })
    .map(|value| value.chars().take(96).collect())
    .unwrap_or_else(|| "local".to_string())
}

/// Whether this IP has hit the failed-attempt limit within the current window.
/// Also prunes expired entries so the map cannot grow unbounded.
pub(crate) async fn auth_rate_limited(state: &AppState, ip: &str) -> bool {
  let window_start = now_millis().saturating_sub(AUTH_FAILURE_WINDOW_MS);
  let mut attempts = state.api_auth_failures.lock().await;
  attempts.retain(|_, values| {
    values.retain(|at| *at >= window_start);
    !values.is_empty()
  });
  attempts.get(ip).is_some_and(|values| values.len() >= AUTH_FAILURE_LIMIT)
}

/// Record one failed auth attempt from this IP.
pub(crate) async fn record_auth_failure(state: &AppState, ip: &str) {
  let mut attempts = state.api_auth_failures.lock().await;
  attempts.entry(ip.to_string()).or_default().push(now_millis());
}

/// Resolve a full `prefix.secret` token (plus username + machine id) to a
/// [`Principal`], or an auth error. Rate-limits and records credential
/// failures per IP.
pub async fn authenticate(
  state: &AppState,
  full_token: &str,
  username: Option<&str>,
  machine_id: Option<&str>,
  ip: &str,
) -> Result<Principal, AppError> {
  if auth_rate_limited(state, ip).await {
    return Err(AppError::TooManyRequests);
  }
  match authenticate_inner(state, full_token, username, machine_id).await {
    // A bad credential (unknown/expired token, wrong username, wrong machine)
    // counts toward the rate limit; authorization failures (403) do not, so a
    // valid-but-unentitled client polling can't lock itself out.
    Err(AppError::Unauthorized) => {
      record_auth_failure(state, ip).await;
      Err(AppError::Unauthorized)
    }
    other => other,
  }
}

async fn authenticate_inner(
  state: &AppState,
  full_token: &str,
  username: Option<&str>,
  machine_id: Option<&str>,
) -> Result<Principal, AppError> {
  let (prefix, secret) =
    split_token(full_token).ok_or(AppError::Unauthorized)?;
  let pool = &state.db.conn;

  let token = repo::tokens::find_by_prefix(pool, prefix)
    .await?
    .ok_or(AppError::Unauthorized)?;
  if token.revoked != 0 {
    return Err(AppError::Unauthorized);
  }
  if token.expires_at.is_some_and(|exp| exp <= now_millis()) {
    return Err(AppError::Unauthorized);
  }
  if !verify_secret(secret, &token.token_hash) {
    return Err(AppError::Unauthorized);
  }

  let device =
    repo::devices::find_by_id(pool, &token.tenant_id, &token.device_id)
      .await?
      .ok_or(AppError::Unauthorized)?;
  if device.revoked != 0 {
    return Err(AppError::Unauthorized);
  }

  // Machine-id gate: a token binds to the first machine it is seen with, and
  // only that machine may use it thereafter.
  enforce_machine_binding(pool, &device, machine_id).await?;

  let user = repo::users::find_by_id(pool, &device.tenant_id, &device.user_id)
    .await?
    .ok_or(AppError::Unauthorized)?;
  if user.disabled != 0 {
    return Err(AppError::Forbidden);
  }
  // Username gate: the presented username must be the device owner's email.
  if !username.is_some_and(|name| name.eq_ignore_ascii_case(user.email.trim()))
  {
    return Err(AppError::Unauthorized);
  }
  let override_rows =
    repo::scopes::list_for_device(pool, &device.tenant_id, &device.id).await?;
  let book_access = override_rows
    .into_iter()
    .map(|row| (row.book_id, AccessLevel::parse(&row.access)))
    .collect();
  let default_access = AccessLevel::parse(&device.default_access);

  // The stored role is admin-vs-user; whether the caller may sync their own
  // library (`personal_sync`) is the injected hook's answer, which grants it
  // unless an override says otherwise. Admins always have it.
  let role = Role::parse(&user.role);
  let personal_sync = role.is_admin()
    || state
      .entitlements
      .resolve(crate::ext::EntCtx {
        tenant_id: &user.tenant_id,
        user_id: &user.id,
        is_admin: role.is_admin(),
      })
      .await
      .personal_sync;

  // Best-effort activity timestamps; failures must not block the request.
  let _ = repo::tokens::touch_last_used(pool, &token.id).await;
  let _ = repo::devices::touch_last_seen(pool, &device.id).await;

  Ok(Principal {
    tenant_id: user.tenant_id,
    user_id: user.id,
    device_id: device.id,
    role,
    personal_sync,
    read_only: !default_access.can_write(),
    progress_sync_denied: !default_access.can_write(),
    default_access,
    book_access,
  })
}

/// Enforce (and, on first use, establish) the device's machine binding. The
/// machine id is required: without it the server cannot lock the token to a
/// machine, so a missing/blank header is rejected.
async fn enforce_machine_binding(
  db: &sea_orm::DatabaseConnection,
  device: &repo::devices::DeviceRow,
  machine_id: Option<&str>,
) -> Result<(), AppError> {
  let machine_id = machine_id.ok_or(AppError::Unauthorized)?;
  match device.machine_id.as_deref() {
    // Already bound: only the bound machine may use this token.
    Some(bound) => {
      if bound == machine_id {
        Ok(())
      } else {
        Err(AppError::Unauthorized)
      }
    }
    // First use: bind atomically. If a concurrent request bound it first, the
    // authoritative value must still be ours, or this request is rejected.
    None => {
      let bound = repo::devices::bind_machine_id(
        db,
        &device.tenant_id,
        &device.id,
        machine_id,
      )
      .await?;
      if bound {
        return Ok(());
      }
      let current =
        repo::devices::find_by_id(db, &device.tenant_id, &device.id)
          .await?
          .and_then(|d| d.machine_id);
      if current.as_deref() == Some(machine_id) {
        Ok(())
      } else {
        Err(AppError::Unauthorized)
      }
    }
  }
}
