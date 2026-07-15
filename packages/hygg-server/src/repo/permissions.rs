use sea_orm::sea_query::OnConflict;
use sea_orm::*;
use serde::Serialize;

use crate::auth::AccessLevel;
use crate::auth::doc_access::{Grant, ResolveInput, resolve};
use crate::entity::document_permissions;
use crate::util::{new_id, now_millis};

#[derive(sea_orm::FromQueryResult, Serialize, Clone, Debug)]
pub struct PermissionRow {
  pub subject_type: String,
  pub subject_id: String,
  pub target_type: String,
  pub target_id: String,
  pub access: String,
}

/// Upsert a grant: `(subject_type, subject_id)` is `user`/`group`,
/// `(target_type, target_id)` is `document`(content hash)/`directory`(id).
#[allow(clippy::too_many_arguments)]
pub async fn set(
  db: &DatabaseConnection,
  tenant_id: &str,
  organization_id: &str,
  subject_type: &str,
  subject_id: &str,
  target_type: &str,
  target_id: &str,
  access: &str,
) -> Result<(), DbErr> {
  let now = now_millis();
  document_permissions::Entity::insert(document_permissions::ActiveModel {
    id: Set(new_id()),
    tenant_id: Set(tenant_id.to_owned()),
    organization_id: Set(organization_id.to_owned()),
    subject_type: Set(subject_type.to_owned()),
    subject_id: Set(subject_id.to_owned()),
    target_type: Set(target_type.to_owned()),
    target_id: Set(target_id.to_owned()),
    access: Set(access.to_owned()),
    created_at: Set(now),
    updated_at: Set(now),
  })
  .on_conflict(
    OnConflict::columns([
      document_permissions::Column::TenantId,
      document_permissions::Column::OrganizationId,
      document_permissions::Column::SubjectType,
      document_permissions::Column::SubjectId,
      document_permissions::Column::TargetType,
      document_permissions::Column::TargetId,
    ])
    .update_columns([
      document_permissions::Column::Access,
      document_permissions::Column::UpdatedAt,
    ])
    .to_owned(),
  )
  .exec_without_returning(db)
  .await?;
  Ok(())
}

pub async fn remove(
  db: &DatabaseConnection,
  tenant_id: &str,
  organization_id: &str,
  subject_type: &str,
  subject_id: &str,
  target_type: &str,
  target_id: &str,
) -> Result<bool, DbErr> {
  let result = document_permissions::Entity::delete_many()
    .filter(document_permissions::Column::TenantId.eq(tenant_id))
    .filter(document_permissions::Column::OrganizationId.eq(organization_id))
    .filter(document_permissions::Column::SubjectType.eq(subject_type))
    .filter(document_permissions::Column::SubjectId.eq(subject_id))
    .filter(document_permissions::Column::TargetType.eq(target_type))
    .filter(document_permissions::Column::TargetId.eq(target_id))
    .exec(db)
    .await?;
  Ok(result.rows_affected > 0)
}

pub async fn list_for_org(
  db: &DatabaseConnection,
  tenant_id: &str,
  organization_id: &str,
) -> Result<Vec<PermissionRow>, DbErr> {
  document_permissions::Entity::find()
    .select_only()
    .column(document_permissions::Column::SubjectType)
    .column(document_permissions::Column::SubjectId)
    .column(document_permissions::Column::TargetType)
    .column(document_permissions::Column::TargetId)
    .column(document_permissions::Column::Access)
    .filter(document_permissions::Column::TenantId.eq(tenant_id))
    .filter(document_permissions::Column::OrganizationId.eq(organization_id))
    .into_model::<PermissionRow>()
    .all(db)
    .await
}

/// Resolve a member's effective access to one organization document. Owners and
/// admins pass `privileged = true` and always get read/write. Otherwise the
/// org's grants and directory tree feed the pure resolver.
#[allow(clippy::too_many_arguments)]
pub async fn effective_access(
  db: &DatabaseConnection,
  tenant_id: &str,
  organization_id: &str,
  user_id: &str,
  privileged: bool,
  org_default: AccessLevel,
  book_hash: &str,
  book_directory_id: Option<&str>,
) -> Result<AccessLevel, DbErr> {
  if privileged {
    return Ok(AccessLevel::ReadWrite);
  }
  let group_ids = crate::repo::groups::group_ids_for_user(
    db,
    tenant_id,
    organization_id,
    user_id,
  )
  .await?;
  let ancestors = match book_directory_id {
    Some(dir) => {
      let dirs =
        crate::repo::directories::list_for_org(db, tenant_id, organization_id)
          .await?;
      crate::repo::directories::ancestor_ids(&dirs, dir)
    }
    None => Vec::new(),
  };
  let grants: Vec<Grant> = list_for_org(db, tenant_id, organization_id)
    .await?
    .iter()
    .filter_map(|row| to_grant(row, user_id, &group_ids))
    .collect();
  Ok(resolve(&ResolveInput {
    privileged: false,
    org_default,
    book_hash,
    ancestor_dir_ids: &ancestors,
    grants: &grants,
  }))
}

/// Keep only grants that apply to this user (their own or one of their groups)
/// and translate them into the resolver's `Grant`.
fn to_grant(
  row: &PermissionRow,
  user_id: &str,
  group_ids: &[String],
) -> Option<Grant> {
  let subject_is_user = match row.subject_type.as_str() {
    "user" if row.subject_id == user_id => true,
    "group" if group_ids.iter().any(|g| g == &row.subject_id) => false,
    _ => return None,
  };
  let target_is_document = match row.target_type.as_str() {
    "document" => true,
    "directory" => false,
    _ => return None,
  };
  Some(Grant {
    subject_is_user,
    target_is_document,
    target_id: row.target_id.clone(),
    access: AccessLevel::parse(&row.access),
  })
}
