use sea_orm::sea_query::OnConflict;
use sea_orm::*;
use serde::Serialize;

use crate::entity::{org_group_members, org_groups, users};
use crate::util::{new_id, now_millis};

#[derive(Serialize, Clone, Debug)]
pub struct GroupRow {
  pub id: String,
  pub organization_id: String,
  pub name: String,
  pub created_at: i64,
  pub updated_at: i64,
}

impl From<org_groups::Model> for GroupRow {
  fn from(model: org_groups::Model) -> Self {
    Self {
      id: model.id,
      organization_id: model.organization_id,
      name: model.name,
      created_at: model.created_at,
      updated_at: model.updated_at,
    }
  }
}

#[derive(sea_orm::FromQueryResult, Serialize, Clone, Debug)]
pub struct GroupMember {
  pub user_id: String,
  pub email: String,
  pub display_name: String,
}

pub async fn create(
  db: &DatabaseConnection,
  tenant_id: &str,
  organization_id: &str,
  name: &str,
) -> Result<String, DbErr> {
  let id = new_id();
  let now = now_millis();
  org_groups::Entity::insert(org_groups::ActiveModel {
    id: Set(id.clone()),
    tenant_id: Set(tenant_id.to_owned()),
    organization_id: Set(organization_id.to_owned()),
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
) -> Result<Vec<GroupRow>, DbErr> {
  Ok(
    org_groups::Entity::find()
      .filter(org_groups::Column::TenantId.eq(tenant_id))
      .filter(org_groups::Column::OrganizationId.eq(organization_id))
      .order_by_asc(org_groups::Column::Name)
      .all(db)
      .await?
      .into_iter()
      .map(GroupRow::from)
      .collect(),
  )
}

pub async fn delete(
  db: &DatabaseConnection,
  tenant_id: &str,
  group_id: &str,
) -> Result<bool, DbErr> {
  let result = org_groups::Entity::delete_many()
    .filter(org_groups::Column::TenantId.eq(tenant_id))
    .filter(org_groups::Column::Id.eq(group_id))
    .exec(db)
    .await?;
  Ok(result.rows_affected > 0)
}

pub async fn add_member(
  db: &DatabaseConnection,
  tenant_id: &str,
  group_id: &str,
  user_id: &str,
) -> Result<bool, DbErr> {
  let affected =
    org_group_members::Entity::insert(org_group_members::ActiveModel {
      id: Set(new_id()),
      tenant_id: Set(tenant_id.to_owned()),
      group_id: Set(group_id.to_owned()),
      user_id: Set(user_id.to_owned()),
      created_at: Set(now_millis()),
    })
    .on_conflict(
      OnConflict::columns([
        org_group_members::Column::TenantId,
        org_group_members::Column::GroupId,
        org_group_members::Column::UserId,
      ])
      .do_nothing()
      .to_owned(),
    )
    .exec_without_returning(db)
    .await?;
  Ok(affected > 0)
}

pub async fn remove_member(
  db: &DatabaseConnection,
  tenant_id: &str,
  group_id: &str,
  user_id: &str,
) -> Result<bool, DbErr> {
  let result = org_group_members::Entity::delete_many()
    .filter(org_group_members::Column::TenantId.eq(tenant_id))
    .filter(org_group_members::Column::GroupId.eq(group_id))
    .filter(org_group_members::Column::UserId.eq(user_id))
    .exec(db)
    .await?;
  Ok(result.rows_affected > 0)
}

pub async fn list_members(
  db: &DatabaseConnection,
  tenant_id: &str,
  group_id: &str,
) -> Result<Vec<GroupMember>, DbErr> {
  org_group_members::Entity::find()
    .select_only()
    .column_as(users::Column::Id, "user_id")
    .column(users::Column::Email)
    .column(users::Column::DisplayName)
    .join(JoinType::InnerJoin, org_group_members::Relation::Users.def())
    .filter(org_group_members::Column::TenantId.eq(tenant_id))
    .filter(org_group_members::Column::GroupId.eq(group_id))
    .filter(users::Column::TenantId.eq(tenant_id))
    .order_by_asc(users::Column::Email)
    .into_model::<GroupMember>()
    .all(db)
    .await
}

/// The ids of every group in the org that `user_id` belongs to (for access
/// resolution).
pub async fn group_ids_for_user(
  db: &DatabaseConnection,
  tenant_id: &str,
  organization_id: &str,
  user_id: &str,
) -> Result<Vec<String>, DbErr> {
  org_groups::Entity::find()
    .select_only()
    .column(org_groups::Column::Id)
    .join(JoinType::InnerJoin, org_groups::Relation::OrgGroupMembers.def())
    .filter(org_groups::Column::TenantId.eq(tenant_id))
    .filter(org_groups::Column::OrganizationId.eq(organization_id))
    .filter(org_group_members::Column::TenantId.eq(tenant_id))
    .filter(org_group_members::Column::UserId.eq(user_id))
    .into_tuple::<String>()
    .all(db)
    .await
}
