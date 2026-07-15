use sea_orm::*;

use crate::entity::organizations;
use crate::util::now_millis;

/// Set the org-wide default permission applied to members on org-owned
/// documents. `access` must already be normalized (`none|read|read_write`).
pub async fn set_default_access(
  db: &DatabaseConnection,
  tenant_id: &str,
  organization_id: &str,
  access: &str,
) -> Result<bool, DbErr> {
  let result = organizations::Entity::update_many()
    .set(organizations::ActiveModel {
      default_access: Set(access.to_owned()),
      updated_at: Set(now_millis()),
      ..Default::default()
    })
    .filter(organizations::Column::TenantId.eq(tenant_id))
    .filter(organizations::Column::Id.eq(organization_id))
    .exec(db)
    .await?;
  Ok(result.rows_affected > 0)
}
