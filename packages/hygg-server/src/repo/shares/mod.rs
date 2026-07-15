//! Peer document shares: one user grants another user (by their tenant-unique
//! email) access to a personal document they own. The book itself stays a
//! single owner-owned row — a share is a grant, not a copy. A share is
//! `pending` until the recipient accepts (then `accepted`) or declines
//! (`declined`); the sender can `revoke` a pending/accepted share.
//!
//! Counting is directional: outgoing "active" = `pending`+`accepted` (against
//! the sender's cap); incoming "active" = `accepted` (against the recipient's
//! cap). Declined/revoked shares free the slot and can be re-shared. The
//! outbox/inbox list queries and directional counts live in [`lists`].

use sea_orm::*;

use crate::auth::AccessLevel;
use crate::entity::document_shares;
use crate::util::{new_id, now_millis};

mod lists;
pub use lists::*;

pub const PENDING: &str = "pending";
pub const ACCEPTED: &str = "accepted";
pub const DECLINED: &str = "declined";
pub const REVOKED: &str = "revoked";

/// The bare ownership/state facts of a share, for authorizing an accept /
/// decline / revoke against the acting user.
#[derive(Debug, Clone)]
pub struct ShareMeta {
  pub content_hash: String,
  pub from_user_id: String,
  pub to_user_id: String,
  pub status: String,
}

impl From<document_shares::Model> for ShareMeta {
  fn from(model: document_shares::Model) -> Self {
    Self {
      content_hash: model.content_hash,
      from_user_id: model.from_user_id,
      to_user_id: model.to_user_id,
      status: model.status,
    }
  }
}

/// Outcome of a create request, so the UI can report it precisely.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateOutcome {
  Created,
  Reactivated,
  AlreadyActive,
}

/// The access level of an *accepted* share of `content_hash` to `to_user_id`,
/// or `None` when there is no accepted share (used by access resolution).
pub async fn accepted_access(
  db: &DatabaseConnection,
  tenant_id: &str,
  content_hash: &str,
  to_user_id: &str,
) -> Result<AccessLevel, DbErr> {
  let row = document_shares::Entity::find()
    .filter(document_shares::Column::TenantId.eq(tenant_id))
    .filter(document_shares::Column::ContentHash.eq(content_hash))
    .filter(document_shares::Column::ToUserId.eq(to_user_id))
    .filter(document_shares::Column::Status.eq(ACCEPTED))
    .one(db)
    .await?;
  Ok(row.map(|r| AccessLevel::parse(&r.access)).unwrap_or(AccessLevel::None))
}

/// Whether an active (pending or accepted) share of `content_hash` to
/// `to_user_id` already exists — a re-submit of the same share is a no-op and
/// must not be blocked by the sender's cap.
pub async fn active_share_exists(
  db: &DatabaseConnection,
  tenant_id: &str,
  content_hash: &str,
  to_user_id: &str,
) -> Result<bool, DbErr> {
  let count = document_shares::Entity::find()
    .filter(document_shares::Column::TenantId.eq(tenant_id))
    .filter(document_shares::Column::ContentHash.eq(content_hash))
    .filter(document_shares::Column::ToUserId.eq(to_user_id))
    .filter(document_shares::Column::Status.is_in([PENDING, ACCEPTED]))
    .count(db)
    .await?;
  Ok(count > 0)
}

/// Create a new pending share, or reactivate a previously declined/revoked one.
/// A share already pending/accepted is left untouched (`AlreadyActive`).
pub async fn create(
  db: &DatabaseConnection,
  tenant_id: &str,
  content_hash: &str,
  from_user_id: &str,
  to_user_id: &str,
  access: &str,
) -> Result<CreateOutcome, DbErr> {
  let existing = document_shares::Entity::find()
    .filter(document_shares::Column::TenantId.eq(tenant_id))
    .filter(document_shares::Column::ContentHash.eq(content_hash))
    .filter(document_shares::Column::ToUserId.eq(to_user_id))
    .one(db)
    .await?;
  let now = now_millis();
  match existing {
    Some(row) if row.status == PENDING || row.status == ACCEPTED => {
      Ok(CreateOutcome::AlreadyActive)
    }
    Some(row) => {
      document_shares::Entity::update_many()
        .set(document_shares::ActiveModel {
          from_user_id: Set(from_user_id.to_owned()),
          access: Set(access.to_owned()),
          status: Set(PENDING.to_owned()),
          created_at: Set(now),
          updated_at: Set(now),
          responded_at: Set(None),
          ..Default::default()
        })
        .filter(document_shares::Column::Id.eq(row.id))
        .exec(db)
        .await?;
      Ok(CreateOutcome::Reactivated)
    }
    None => {
      document_shares::Entity::insert(document_shares::ActiveModel {
        id: Set(new_id()),
        tenant_id: Set(tenant_id.to_owned()),
        content_hash: Set(content_hash.to_owned()),
        from_user_id: Set(from_user_id.to_owned()),
        to_user_id: Set(to_user_id.to_owned()),
        access: Set(access.to_owned()),
        status: Set(PENDING.to_owned()),
        created_at: Set(now),
        updated_at: Set(now),
        responded_at: NotSet,
      })
      .exec(db)
      .await?;
      Ok(CreateOutcome::Created)
    }
  }
}

/// Look up a share by id (tenant-scoped) for authorization checks.
pub async fn find(
  db: &DatabaseConnection,
  tenant_id: &str,
  id: &str,
) -> Result<Option<ShareMeta>, DbErr> {
  Ok(
    document_shares::Entity::find()
      .filter(document_shares::Column::TenantId.eq(tenant_id))
      .filter(document_shares::Column::Id.eq(id))
      .one(db)
      .await?
      .map(ShareMeta::from),
  )
}

/// Recipient accepts a pending share. Returns whether a row transitioned.
pub async fn accept(
  db: &DatabaseConnection,
  tenant_id: &str,
  id: &str,
  to_user_id: &str,
) -> Result<bool, DbErr> {
  set_status_by(
    db,
    tenant_id,
    id,
    document_shares::Column::ToUserId,
    to_user_id,
    PENDING,
    ACCEPTED,
  )
  .await
}

/// Recipient declines a pending share.
pub async fn decline(
  db: &DatabaseConnection,
  tenant_id: &str,
  id: &str,
  to_user_id: &str,
) -> Result<bool, DbErr> {
  set_status_by(
    db,
    tenant_id,
    id,
    document_shares::Column::ToUserId,
    to_user_id,
    PENDING,
    DECLINED,
  )
  .await
}

/// Sender revokes a share they created (pending or already accepted), removing
/// the recipient's access.
pub async fn revoke(
  db: &DatabaseConnection,
  tenant_id: &str,
  id: &str,
  from_user_id: &str,
) -> Result<bool, DbErr> {
  let now = now_millis();
  let result = document_shares::Entity::update_many()
    .set(responded(REVOKED, now))
    .filter(document_shares::Column::TenantId.eq(tenant_id))
    .filter(document_shares::Column::Id.eq(id))
    .filter(document_shares::Column::FromUserId.eq(from_user_id))
    .filter(document_shares::Column::Status.is_in([PENDING, ACCEPTED]))
    .exec(db)
    .await?;
  Ok(result.rows_affected > 0)
}

/// The recipient removes ("unshares") a document shared with them, dropping it
/// from their library (accepted → declined, freeing the slot). Keyed by content
/// hash since that is what the library card knows. Returns whether it changed.
pub async fn leave_by_hash(
  db: &DatabaseConnection,
  tenant_id: &str,
  content_hash: &str,
  to_user_id: &str,
) -> Result<bool, DbErr> {
  let now = now_millis();
  let result = document_shares::Entity::update_many()
    .set(responded(DECLINED, now))
    .filter(document_shares::Column::TenantId.eq(tenant_id))
    .filter(document_shares::Column::ContentHash.eq(content_hash))
    .filter(document_shares::Column::ToUserId.eq(to_user_id))
    .filter(document_shares::Column::Status.eq(ACCEPTED))
    .exec(db)
    .await?;
  Ok(result.rows_affected > 0)
}

async fn set_status_by(
  db: &DatabaseConnection,
  tenant_id: &str,
  id: &str,
  actor_col: document_shares::Column,
  actor_id: &str,
  from_status: &str,
  to_status: &str,
) -> Result<bool, DbErr> {
  let now = now_millis();
  let result = document_shares::Entity::update_many()
    .set(responded(to_status, now))
    .filter(document_shares::Column::TenantId.eq(tenant_id))
    .filter(document_shares::Column::Id.eq(id))
    .filter(actor_col.eq(actor_id))
    .filter(document_shares::Column::Status.eq(from_status))
    .exec(db)
    .await?;
  Ok(result.rows_affected > 0)
}

/// The columns every status transition writes: the new status, stamped as
/// responded at `now`.
fn responded(status: &str, now: i64) -> document_shares::ActiveModel {
  document_shares::ActiveModel {
    status: Set(status.to_owned()),
    responded_at: Set(Some(now)),
    updated_at: Set(now),
    ..Default::default()
  }
}
