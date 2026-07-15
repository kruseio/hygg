use sea_orm::*;
use serde::Serialize;

use crate::entity::directories;
use crate::util::{new_id, now_millis};

#[derive(Serialize, Clone, Debug)]
pub struct DirectoryRow {
  pub id: String,
  pub organization_id: String,
  pub parent_id: Option<String>,
  pub name: String,
  pub created_at: i64,
  pub updated_at: i64,
}

impl From<directories::Model> for DirectoryRow {
  fn from(model: directories::Model) -> Self {
    Self {
      id: model.id,
      organization_id: model.organization_id,
      parent_id: model.parent_id,
      name: model.name,
      created_at: model.created_at,
      updated_at: model.updated_at,
    }
  }
}

pub async fn create(
  db: &DatabaseConnection,
  tenant_id: &str,
  organization_id: &str,
  parent_id: Option<&str>,
  name: &str,
) -> Result<String, DbErr> {
  let id = new_id();
  let now = now_millis();
  directories::Entity::insert(directories::ActiveModel {
    id: Set(id.clone()),
    tenant_id: Set(tenant_id.to_owned()),
    organization_id: Set(organization_id.to_owned()),
    parent_id: Set(parent_id.map(str::to_owned)),
    name: Set(name.trim().to_owned()),
    created_at: Set(now),
    updated_at: Set(now),
  })
  .exec(db)
  .await?;
  Ok(id)
}

pub async fn list_for_org(
  db: &DatabaseConnection,
  tenant_id: &str,
  organization_id: &str,
) -> Result<Vec<DirectoryRow>, DbErr> {
  Ok(
    directories::Entity::find()
      .filter(directories::Column::TenantId.eq(tenant_id))
      .filter(directories::Column::OrganizationId.eq(organization_id))
      .order_by_asc(directories::Column::Name)
      .all(db)
      .await?
      .into_iter()
      .map(DirectoryRow::from)
      .collect(),
  )
}

pub async fn rename(
  db: &DatabaseConnection,
  tenant_id: &str,
  directory_id: &str,
  name: &str,
) -> Result<bool, DbErr> {
  let result = directories::Entity::update_many()
    .set(directories::ActiveModel {
      name: Set(name.trim().to_owned()),
      updated_at: Set(now_millis()),
      ..Default::default()
    })
    .filter(directories::Column::TenantId.eq(tenant_id))
    .filter(directories::Column::Id.eq(directory_id))
    .exec(db)
    .await?;
  Ok(result.rows_affected > 0)
}

/// The directory's own id followed by its ancestors, nearest first. Bounded by
/// the directory count so a malformed parent cycle can't loop forever.
pub fn ancestor_ids(dirs: &[DirectoryRow], start: &str) -> Vec<String> {
  let mut chain = Vec::new();
  let mut current = Some(start.to_string());
  while let Some(id) = current {
    if chain.len() > dirs.len() {
      break;
    }
    let parent =
      dirs.iter().find(|d| d.id == id).and_then(|d| d.parent_id.clone());
    chain.push(id);
    current = parent;
  }
  chain
}
