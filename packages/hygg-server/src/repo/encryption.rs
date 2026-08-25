//! The account encryption marker: read it, and turn encryption on.
//!
//! The marker is per `(tenant, user)`. Only its *public* half lives here — the
//! server holds no key and cannot decrypt a thing. `enable` is idempotent for a
//! resent identical salt but refuses a *different* salt on an already-enabled
//! account, because changing the salt changes the derived key and would strand
//! every document already sealed under the old one.

use sea_orm::*;

use crate::entity::encryption_markers;
use crate::util::{new_id, now_millis};

/// The current marker row for an account, if any.
pub async fn get(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
) -> Result<Option<encryption_markers::Model>, DbErr> {
  encryption_markers::Entity::find()
    .filter(encryption_markers::Column::TenantId.eq(tenant_id))
    .filter(encryption_markers::Column::UserId.eq(user_id))
    .one(db)
    .await
}

/// Whether encrypted uploads are required for this account. The hot-path check
/// the blob and note handlers call on every write, so it stays a single cheap
/// query returning a bool.
pub async fn is_enabled(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
) -> Result<bool, DbErr> {
  Ok(
    get(db, tenant_id, user_id).await?.map(|m| m.enabled != 0).unwrap_or(false),
  )
}

/// The result of an `enable` call.
pub enum EnableOutcome {
  /// Encryption is now on with the given marker.
  Set(encryption_markers::Model),
  /// Refused: the account is already encrypted under a different salt.
  SaltConflict,
}

/// Turn encryption on (or confirm it is on) for an account.
#[allow(clippy::too_many_arguments)]
pub async fn enable(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
  kdf: &str,
  alg: i64,
  salt: &str,
  verifier: &str,
) -> Result<EnableOutcome, DbErr> {
  let now = now_millis();
  if let Some(existing) = get(db, tenant_id, user_id).await? {
    // Already initialized under a different salt: refuse rather than orphan the
    // documents sealed under the first key. An *empty* stored salt means the
    // marker was mandated by the server but not yet initialized by a client, so
    // the first client is allowed to publish its salt here.
    if existing.enabled != 0
      && !existing.salt.is_empty()
      && existing.salt != salt
    {
      return Ok(EnableOutcome::SaltConflict);
    }
    let mut active: encryption_markers::ActiveModel = existing.into();
    active.enabled = Set(1);
    active.kdf = Set(kdf.to_owned());
    active.alg = Set(alg);
    active.salt = Set(salt.to_owned());
    active.verifier = Set(verifier.to_owned());
    active.updated_at = Set(now);
    return Ok(EnableOutcome::Set(active.update(db).await?));
  }
  let model = encryption_markers::ActiveModel {
    id: Set(new_id()),
    tenant_id: Set(tenant_id.to_owned()),
    user_id: Set(user_id.to_owned()),
    enabled: Set(1),
    kdf: Set(kdf.to_owned()),
    alg: Set(alg),
    salt: Set(salt.to_owned()),
    verifier: Set(verifier.to_owned()),
    created_at: Set(now),
    updated_at: Set(now),
  }
  .insert(db)
  .await?;
  Ok(EnableOutcome::Set(model))
}

/// Require encryption for the account *without* a key (the server-side toggle):
/// the marker is turned on, but the salt/verifier stay empty until the first
/// client initializes them. Enforcement is already active (enabled != 0), so
/// plaintext uploads are refused meanwhile — which is what forces a client to
/// run its key-generation wizard. Idempotent, and never disturbs an
/// already-initialized marker's key material.
pub async fn mandate(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
) -> Result<encryption_markers::Model, DbErr> {
  let now = now_millis();
  if let Some(existing) = get(db, tenant_id, user_id).await? {
    let mut active: encryption_markers::ActiveModel = existing.into();
    active.enabled = Set(1);
    active.updated_at = Set(now);
    return active.update(db).await;
  }
  encryption_markers::ActiveModel {
    id: Set(new_id()),
    tenant_id: Set(tenant_id.to_owned()),
    user_id: Set(user_id.to_owned()),
    enabled: Set(1),
    kdf: Set(String::new()),
    alg: Set(0),
    salt: Set(String::new()),
    verifier: Set(String::new()),
    created_at: Set(now),
    updated_at: Set(now),
  }
  .insert(db)
  .await
}

/// Turn encryption off for the account and clear its key material. A disabled
/// account is a clean teardown: once a client has decrypted and re-uploaded its
/// documents as plaintext, the old key is defunct (every device forgets it), so
/// leaving a stale salt/verifier behind would only mislead a later re-enable.
/// Re-enabling is therefore a fresh mandate — a new key is generated. Returns
/// whether a marker existed to disable.
pub async fn disable(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
) -> Result<bool, DbErr> {
  let Some(existing) = get(db, tenant_id, user_id).await? else {
    return Ok(false);
  };
  let mut active: encryption_markers::ActiveModel = existing.into();
  active.enabled = Set(0);
  active.kdf = Set(String::new());
  active.alg = Set(0);
  active.salt = Set(String::new());
  active.verifier = Set(String::new());
  active.updated_at = Set(now_millis());
  active.update(db).await?;
  Ok(true)
}
