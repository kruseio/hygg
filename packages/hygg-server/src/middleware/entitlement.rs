//! The sync-access enforcement point. [`SyncPrincipal`] is a [`Principal`]
//! admitted to the sync API; sync, book and blob handlers take it instead of a
//! bare `Principal`. Access stacks: a user allowed to sync their own library
//! (already resolved in `authn`) is admitted, and so is a user who holds a seat
//! in any organization — per-document access then restricts a seat-only user to
//! that org's documents. A user with neither is rejected with 403 (they can
//! still authenticate via `/me`).
//!
//! By default everyone is admitted; this only ever turns anyone away when an
//! override says so.

use axum::extract::FromRequestParts;
use axum::http::request::Parts;

use crate::auth::Principal;
use crate::error::AppError;
use crate::repo;
use crate::state::AppState;

pub struct SyncPrincipal(pub Principal);

/// The gate as a plain function: admit a `Principal` to the sync API (own
/// library or an org seat), else 403. Shared by the
/// [`SyncPrincipal`] extractor and the SSE `events` handler, which
/// authenticates from query params (an `EventSource` cannot send headers) and
/// so can't use the extractor.
pub async fn admit_sync(
  state: &AppState,
  principal: Principal,
) -> Result<Principal, AppError> {
  if !principal.personal_sync {
    let has_seat = repo::organizations::has_membership(
      &state.db.conn,
      &principal.tenant_id,
      &principal.user_id,
    )
    .await
    .unwrap_or(false);
    if !has_seat {
      // Whoever withheld access explains it; the core has nothing to say
      // about a policy it does not implement.
      return Err(AppError::Denied(
        state
          .entitlements
          .sync_denial(crate::ext::EntCtx {
            tenant_id: &principal.tenant_id,
            user_id: &principal.user_id,
            is_admin: principal.role.is_admin(),
          })
          .await,
      ));
    }
  }
  Ok(principal)
}

impl FromRequestParts<AppState> for SyncPrincipal {
  type Rejection = AppError;

  async fn from_request_parts(
    parts: &mut Parts,
    state: &AppState,
  ) -> Result<Self, Self::Rejection> {
    let principal = Principal::from_request_parts(parts, state).await?;
    Ok(SyncPrincipal(admit_sync(state, principal).await?))
  }
}
