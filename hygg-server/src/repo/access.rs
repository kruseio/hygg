//! Effective per-user access to a document, layered on top of the per-device
//! scope evaluated in [`crate::auth::Principal`]. Personal documents stay
//! owner-only for the library; organization documents resolve through the
//! permission model (org default + directory/group/user grants). Annotation
//! sync is more permissive: a user's own progress/notes on personal (or
//! not-yet-uploaded) documents are always theirs, so only organization
//! documents they cannot read are blocked.
//!
//! Entitlement stacks on top: `entitled_personal` (the caller's personal
//! access) is required for personal documents, while organization documents
//! are covered by the caller's org seat (their membership, already enforced by
//! the resolver). So an org-seat-only user reaches org documents but not their
//! own private library. An organization's seat/device caps come from the
//! injected [`Entitlements`] hook (uncapped on self-host).

use sea_orm::{DatabaseConnection, DbErr};

use crate::auth::AccessLevel;
use crate::ext::{Entitlements, OrgCapCtx};
use crate::repo;

/// Library access (list / download / upload / edit / delete) for a document
/// whose ownership facts are already known.
#[allow(clippy::too_many_arguments)]
pub async fn library(
  db: &DatabaseConnection,
  ent: &dyn Entitlements,
  tenant_id: &str,
  user_id: &str,
  is_admin: bool,
  entitled_personal: bool,
  device_id: Option<&str>,
  owner_user_id: &str,
  organization_id: Option<&str>,
  directory_id: Option<&str>,
  content_hash: &str,
) -> Result<AccessLevel, DbErr> {
  match organization_id {
    None => {
      if owner_user_id == user_id {
        // The owner's own personal library needs their personal entitlement.
        Ok(if entitled_personal {
          AccessLevel::ReadWrite
        } else {
          AccessLevel::None
        })
      } else {
        // A personal document shared directly to this user: their access is
        // whatever the accepted share granted (read or read/write).
        repo::shares::accepted_access(db, tenant_id, content_hash, user_id)
          .await
      }
    }
    Some(org) => {
      resolve_org(
        db,
        ent,
        tenant_id,
        user_id,
        is_admin,
        device_id,
        org,
        content_hash,
        directory_id,
      )
      .await
    }
  }
}

/// Library access for a document identified only by its content hash. Unknown
/// documents resolve to no access.
#[allow(clippy::too_many_arguments)]
pub async fn library_for_hash(
  db: &DatabaseConnection,
  ent: &dyn Entitlements,
  tenant_id: &str,
  user_id: &str,
  is_admin: bool,
  entitled_personal: bool,
  device_id: Option<&str>,
  content_hash: &str,
) -> Result<AccessLevel, DbErr> {
  let Some(meta) =
    repo::books::access_meta(db, tenant_id, content_hash).await?
  else {
    return Ok(AccessLevel::None);
  };
  library(
    db,
    ent,
    tenant_id,
    user_id,
    is_admin,
    entitled_personal,
    device_id,
    &meta.owner_user_id,
    meta.organization_id.as_deref(),
    meta.directory_id.as_deref(),
    content_hash,
  )
  .await
}

/// Whether the user may sync annotations/progress for this document. Their own
/// data on personal (or not-yet-uploaded) documents needs personal entitlement;
/// organization documents they cannot read are blocked.
#[allow(clippy::too_many_arguments)]
pub async fn annotation_readable_for_hash(
  db: &DatabaseConnection,
  ent: &dyn Entitlements,
  tenant_id: &str,
  user_id: &str,
  is_admin: bool,
  entitled_personal: bool,
  device_id: Option<&str>,
  content_hash: &str,
) -> Result<bool, DbErr> {
  let Some(meta) =
    repo::books::access_meta(db, tenant_id, content_hash).await?
  else {
    return Ok(entitled_personal);
  };
  match meta.organization_id.as_deref() {
    // Personal document: the entitled owner may sync their own annotations, and
    // a user the document was shared with may sync *their own* (independent)
    // annotations on it too.
    None => {
      if entitled_personal {
        Ok(true)
      } else {
        Ok(
          repo::shares::accepted_access(db, tenant_id, content_hash, user_id)
            .await?
            .can_read(),
        )
      }
    }
    Some(org) => Ok(
      resolve_org(
        db,
        ent,
        tenant_id,
        user_id,
        is_admin,
        device_id,
        org,
        content_hash,
        meta.directory_id.as_deref(),
      )
      .await?
      .can_read(),
    ),
  }
}

/// Resolve org-document access. The org's seat/device caps come from the
/// entitlements hook and are applied before the permission grants: a capped
/// device (sync only, owners included) or a capped member (past the seat
/// count) is blocked from the org's content — "only N users / N devices may
/// sync". `device_id` is the syncing device (API only; web passes `None`).
#[allow(clippy::too_many_arguments)]
async fn resolve_org(
  db: &DatabaseConnection,
  ent: &dyn Entitlements,
  tenant_id: &str,
  user_id: &str,
  is_admin: bool,
  device_id: Option<&str>,
  organization_id: &str,
  content_hash: &str,
  directory_id: Option<&str>,
) -> Result<AccessLevel, DbErr> {
  if is_admin {
    return Ok(AccessLevel::ReadWrite);
  }
  let Some(role) =
    repo::organizations::user_role(db, tenant_id, organization_id, user_id)
      .await?
  else {
    return Ok(AccessLevel::None);
  };
  let caps = ent
    .org_caps(OrgCapCtx { tenant_id, organization_id, user_id, device_id })
    .await;
  // Device cap (sync only) applies to every device, owners included.
  if caps.device_capped {
    return Ok(AccessLevel::None);
  }
  if role == "owner" {
    return Ok(AccessLevel::ReadWrite);
  }
  // Seat cap for ordinary members.
  if caps.member_capped {
    return Ok(AccessLevel::None);
  }
  let org =
    repo::organizations::find_by_id(db, tenant_id, organization_id).await?;
  let org_default = org
    .map(|org| AccessLevel::parse(&org.default_access))
    .unwrap_or(AccessLevel::None);
  repo::permissions::effective_access(
    db,
    tenant_id,
    organization_id,
    user_id,
    false,
    org_default,
    content_hash,
    directory_id,
  )
  .await
}
