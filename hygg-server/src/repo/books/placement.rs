use sea_orm::*;

use crate::entity::books;
use crate::util::now_millis;

pub async fn move_to_organization(
  db: &DatabaseConnection,
  tenant_id: &str,
  owner_user_id: &str,
  content_hash: &str,
  organization_id: Option<&str>,
) -> Result<bool, DbErr> {
  // Moving between orgs (or back to private) invalidates the directory
  // placement, which is scoped to a single organization.
  let result = books::Entity::update_many()
    .set(books::ActiveModel {
      organization_id: Set(organization_id.map(str::to_owned)),
      directory_id: Set(None),
      updated_at: Set(now_millis()),
      ..Default::default()
    })
    .filter(books::Column::TenantId.eq(tenant_id))
    .filter(books::Column::OwnerUserId.eq(owner_user_id))
    .filter(books::Column::ContentHash.eq(content_hash))
    .exec(db)
    .await?;
  Ok(result.rows_affected > 0)
}

/// Place an org document into a directory (or clear it with `None`). Scoped to
/// the document's organization by the caller.
pub async fn set_directory(
  db: &DatabaseConnection,
  tenant_id: &str,
  content_hash: &str,
  directory_id: Option<&str>,
) -> Result<bool, DbErr> {
  let result = books::Entity::update_many()
    .set(books::ActiveModel {
      directory_id: Set(directory_id.map(str::to_owned)),
      updated_at: Set(now_millis()),
      ..Default::default()
    })
    .filter(books::Column::TenantId.eq(tenant_id))
    .filter(books::Column::ContentHash.eq(content_hash))
    .exec(db)
    .await?;
  Ok(result.rows_affected > 0)
}
