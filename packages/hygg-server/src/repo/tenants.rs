use sea_orm::*;

use crate::entity::tenants;
use crate::util::{new_id, now_millis};

pub async fn find_id_by_slug(
  db: &DatabaseConnection,
  slug: &str,
) -> Result<Option<String>, DbErr> {
  Ok(
    tenants::Entity::find()
      .filter(tenants::Column::Slug.eq(slug))
      .one(db)
      .await?
      .map(|t| t.id),
  )
}

pub async fn insert(
  db: &DatabaseConnection,
  slug: &str,
  name: &str,
) -> Result<String, DbErr> {
  let id = new_id();
  tenants::ActiveModel {
    id: Set(id.clone()),
    slug: Set(slug.to_owned()),
    name: Set(name.to_owned()),
    disabled: Set(0),
    created_at: Set(now_millis()),
  }
  .insert(db)
  .await?;
  Ok(id)
}
