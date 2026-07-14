//! Device endpoints: register a new device (exchanging user credentials for a
//! per-device token) and report the authenticated principal.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use hygg_shared::sync::proto::{
  self, RegisterDeviceRequest, RegisterDeviceResponse,
};

use crate::auth::password::verify_password;
use crate::auth::token::generate_token;
use crate::auth::{Principal, Role};
use crate::bootstrap::DEFAULT_TENANT_SLUG;
use crate::error::{AppError, AppResult};
use crate::middleware::authn::{
  auth_rate_limited, client_ip, record_auth_failure,
};
use crate::repo;
use crate::state::AppState;

/// `POST /api/v1/devices/register` — authenticate a user by email + password
/// and mint a new device with its own API token (returned once). Bad-password
/// attempts are rate-limited per IP so the endpoint can't be used to spray
/// credentials, and the new device is bound to the caller's machine id.
pub async fn register(
  State(state): State<AppState>,
  headers: HeaderMap,
  Json(req): Json<RegisterDeviceRequest>,
) -> AppResult<Json<RegisterDeviceResponse>> {
  let ip = client_ip(&headers);
  if auth_rate_limited(&state, &ip).await {
    return Err(AppError::TooManyRequests);
  }
  match register_inner(&state, req).await {
    Err(AppError::Unauthorized) => {
      record_auth_failure(&state, &ip).await;
      Err(AppError::Unauthorized)
    }
    other => other,
  }
}

async fn register_inner(
  state: &AppState,
  req: RegisterDeviceRequest,
) -> AppResult<Json<RegisterDeviceResponse>> {
  let pool = &state.db.conn;
  let tenant_id = repo::tenants::find_id_by_slug(pool, DEFAULT_TENANT_SLUG)
    .await?
    .ok_or(AppError::Unauthorized)?;

  let user = repo::users::find_by_email(pool, &tenant_id, &req.email)
    .await?
    .ok_or(AppError::Unauthorized)?;
  if user.disabled != 0 {
    return Err(AppError::Forbidden);
  }
  let stored = user.password_hash.as_deref().ok_or(AppError::Unauthorized)?;
  if user.password_enabled == 0 || !verify_password(&req.password, stored) {
    return Err(AppError::Unauthorized);
  }
  // Registration gate, always allowed unless an override says otherwise. The
  // core delegates the whole decision — and its wording — to the hook.
  state
    .entitlements
    .authorize_device_registration(crate::ext::EntCtx {
      tenant_id: &tenant_id,
      user_id: &user.id,
      is_admin: Role::parse(&user.role).is_admin(),
    })
    .await?;

  let device_id = repo::devices::insert(
    pool,
    &tenant_id,
    &user.id,
    &req.device_name,
    &req.platform,
  )
  .await?;
  // Lock the new device to the registering machine, so its token can only be
  // used from here. Blank/absent binds nothing (first authenticated request
  // binds it instead).
  if let Some(machine_id) =
    req.machine_id.as_deref().map(str::trim).filter(|m| !m.is_empty())
  {
    let _ =
      repo::devices::bind_machine_id(pool, &tenant_id, &device_id, machine_id)
        .await;
  }
  let token = generate_token();
  repo::tokens::insert(
    pool,
    &tenant_id,
    &device_id,
    &token.prefix,
    &token.hash,
  )
  .await?;
  crate::web::check_user_orgs(state, &tenant_id, &user.id).await;

  Ok(Json(RegisterDeviceResponse {
    device_id,
    token: token.full,
    tenant_id,
    user_id: user.id,
  }))
}

/// `GET /api/v1/me` — the authenticated principal (requires a valid token).
pub async fn me(
  principal: Principal,
  State(state): State<AppState>,
) -> Json<proto::MeResponse> {
  let mut response = proto::MeResponse::from(&principal);
  // Whatever label the deployment wants shown against the account, if any.
  response.label = state
    .entitlements
    .account_label(crate::ext::EntCtx {
      tenant_id: &principal.tenant_id,
      user_id: &principal.user_id,
      is_admin: principal.role.is_admin(),
    })
    .await;
  Json(response)
}

/// `GET /api/v1/devices` — the caller's devices (their own, by user).
pub async fn list_devices(
  principal: Principal,
  State(state): State<AppState>,
) -> AppResult<Json<Vec<proto::DeviceDto>>> {
  let rows = repo::devices::list_for_user(
    &state.db.conn,
    &principal.tenant_id,
    &principal.user_id,
  )
  .await?;
  Ok(Json(rows.into_iter().map(Into::into).collect()))
}

/// `DELETE /api/v1/devices/{id}` — revoke one of the caller's own devices and
/// its tokens. 404 if it does not exist or belongs to another user.
pub async fn revoke_device(
  principal: Principal,
  State(state): State<AppState>,
  Path(device_id): Path<String>,
) -> AppResult<Json<proto::RevokeDeviceResponse>> {
  let revoked = repo::devices::revoke(
    &state.db.conn,
    &principal.tenant_id,
    &principal.user_id,
    &device_id,
  )
  .await?;
  if !revoked {
    return Err(AppError::NotFound);
  }
  Ok(Json(proto::RevokeDeviceResponse { revoked: device_id }))
}
