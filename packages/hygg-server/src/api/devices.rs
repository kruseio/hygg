//! Device endpoints: register a new device (exchanging user credentials for a
//! per-device token) and report the authenticated principal.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use hygg_shared::sync::proto::{
  self, RegisterDeviceRequest, RegisterDeviceResponse, SignupRequest,
  SignupResponse,
};

use crate::auth::password::{hash_password, verify_password};
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

  let (device_id, token) = mint_device(
    state,
    &tenant_id,
    &user.id,
    &req.device_name,
    &req.platform,
    req.machine_id.as_deref(),
  )
  .await?;

  Ok(Json(RegisterDeviceResponse {
    device_id,
    token,
    tenant_id,
    user_id: user.id,
  }))
}

/// Create a device for `user_id` and mint its API token (returned once),
/// binding it to `machine_id` when one is supplied. Shared by device
/// registration and signup — both end by handing a fresh device token back.
async fn mint_device(
  state: &AppState,
  tenant_id: &str,
  user_id: &str,
  device_name: &str,
  platform: &str,
  machine_id: Option<&str>,
) -> AppResult<(String, String)> {
  let pool = &state.db.conn;
  let device_id =
    repo::devices::insert(pool, tenant_id, user_id, device_name, platform)
      .await?;
  // Lock the new device to the registering machine, so its token can only be
  // used from here. Blank/absent binds nothing (first authenticated request
  // binds it instead).
  if let Some(machine_id) = machine_id.map(str::trim).filter(|m| !m.is_empty())
  {
    let _ =
      repo::devices::bind_machine_id(pool, tenant_id, &device_id, machine_id)
        .await;
  }
  let token = generate_token();
  repo::tokens::insert(pool, tenant_id, &device_id, &token.prefix, &token.hash)
    .await?;
  crate::web::check_user_orgs(state, tenant_id, user_id).await;
  Ok((device_id, token.full))
}

/// `POST /api/v1/signup` — create an account and mint its first device token in
/// one call, so a client can go from "no account" to "connected" without the
/// web signup form. Rate-limited per IP like registration; the password is
/// hashed with Argon2id and never returned. The account-creation gate is
/// delegated to the entitlements hook, so a deployment can close registration.
pub async fn signup(
  State(state): State<AppState>,
  headers: HeaderMap,
  Json(req): Json<SignupRequest>,
) -> AppResult<Json<SignupResponse>> {
  let ip = client_ip(&headers);
  if auth_rate_limited(&state, &ip).await {
    return Err(AppError::TooManyRequests);
  }

  let pool = &state.db.conn;
  let email = req.email.trim().to_lowercase();
  if email.is_empty() {
    return Err(AppError::BadRequest("Email is required".into()));
  }
  if let Some(msg) = crate::web::password_complexity_error(&req.password) {
    return Err(AppError::BadRequest(msg.into()));
  }
  let tenant_id = repo::tenants::find_id_by_slug(pool, DEFAULT_TENANT_SLUG)
    .await?
    .ok_or(AppError::Internal)?;

  // Account-creation gate (open by default). Keyed on the tenant — no user
  // exists yet — so `is_admin` is meaningless here and passed as false.
  state
    .entitlements
    .authorize_signup(crate::ext::EntCtx {
      tenant_id: &tenant_id,
      user_id: "",
      is_admin: false,
    })
    .await?;

  let hash = hash_password(&req.password).map_err(|_| AppError::Internal)?;
  let display_name =
    if req.display_name.trim().is_empty() { &email } else { &req.display_name };
  // A unique-index violation on the email surfaces as an insert error; report
  // it as a conflict rather than a generic failure so the client can say so.
  let user_id = repo::users::insert(
    pool,
    &tenant_id,
    &email,
    display_name,
    Some(&hash),
    "user",
  )
  .await
  .map_err(|_| AppError::Conflict("Account already exists".into()))?;

  let (device_id, token) = mint_device(
    &state,
    &tenant_id,
    &user_id,
    &req.device_name,
    &req.platform,
    req.machine_id.as_deref(),
  )
  .await?;

  Ok(Json(SignupResponse { device_id, token, tenant_id, user_id }))
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
