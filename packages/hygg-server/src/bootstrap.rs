//! First-run bootstrap: guarantee a default tenant exists and, when the
//! `ADMIN_BOOTSTRAP_*` env vars are set and the admin is absent, create the
//! initial admin user. Idempotent — safe to run on every startup.

use crate::auth::password::hash_password;
use crate::error::AppResult;
use crate::repo;
use crate::state::AppState;

/// Slug of the tenant used for single-tenant self-hosting.
pub const DEFAULT_TENANT_SLUG: &str = "default";

pub async fn ensure_bootstrap(state: &AppState) -> AppResult<()> {
  let tenant_id = ensure_default_tenant(state).await?;

  let email = std::env::var("ADMIN_BOOTSTRAP_EMAIL").unwrap_or_default();
  let password = std::env::var("ADMIN_BOOTSTRAP_PASSWORD").unwrap_or_default();
  if email.is_empty() || password.is_empty() {
    if repo::users::count_admins(&state.db.conn, &tenant_id).await? == 0 {
      tracing::warn!(
        "no admin user exists; set ADMIN_BOOTSTRAP_EMAIL and \
         ADMIN_BOOTSTRAP_PASSWORD in .env, then restart"
      );
    }
    return Ok(());
  }

  let pool = &state.db.conn;
  let hash = hash_password(&password)?;
  match repo::users::find_by_email(pool, &tenant_id, &email).await? {
    Some(user) => {
      repo::users::set_password_hash(pool, &tenant_id, &user.id, &hash).await?;
      if user.role != "admin" {
        repo::users::set_role(pool, &tenant_id, &user.id, "admin").await?;
      }
      if user.disabled != 0 {
        repo::users::set_disabled(pool, &tenant_id, &user.id, false).await?;
      }
      tracing::info!("ensured bootstrap admin user {email}");
    }
    None => {
      repo::users::insert(
        pool,
        &tenant_id,
        &email,
        "Admin",
        Some(&hash),
        "admin",
      )
      .await?;
      tracing::info!("bootstrapped admin user {email}");
    }
  }
  Ok(())
}

/// Return the default tenant's id, creating it if necessary.
pub async fn ensure_default_tenant(state: &AppState) -> AppResult<String> {
  let pool = &state.db.conn;
  if let Some(id) =
    repo::tenants::find_id_by_slug(pool, DEFAULT_TENANT_SLUG).await?
  {
    return Ok(id);
  }
  let id = repo::tenants::insert(pool, DEFAULT_TENANT_SLUG, "Default").await?;
  Ok(id)
}
