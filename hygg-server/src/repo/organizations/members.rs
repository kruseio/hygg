use sea_orm::sea_query::OnConflict;
use sea_orm::*;

use super::{OrganizationMember, normalized_member_role};
use crate::entity::{organization_members, users};
use crate::util::{new_id, now_millis};

pub async fn add_member(
  db: &DatabaseConnection,
  tenant_id: &str,
  organization_id: &str,
  user_id: &str,
  role: &str,
) -> Result<bool, DbErr> {
  let affected =
    organization_members::Entity::insert(organization_members::ActiveModel {
      id: Set(new_id()),
      tenant_id: Set(tenant_id.to_owned()),
      organization_id: Set(organization_id.to_owned()),
      user_id: Set(user_id.to_owned()),
      role: Set(normalized_member_role(role).to_owned()),
      created_at: Set(now_millis()),
    })
    .on_conflict(
      OnConflict::columns([
        organization_members::Column::TenantId,
        organization_members::Column::OrganizationId,
        organization_members::Column::UserId,
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
  organization_id: &str,
  user_id: &str,
) -> Result<bool, DbErr> {
  let result = organization_members::Entity::delete_many()
    .filter(organization_members::Column::TenantId.eq(tenant_id))
    .filter(organization_members::Column::OrganizationId.eq(organization_id))
    .filter(organization_members::Column::UserId.eq(user_id))
    .exec(db)
    .await?;
  Ok(result.rows_affected > 0)
}

pub async fn set_member_role(
  db: &DatabaseConnection,
  tenant_id: &str,
  organization_id: &str,
  user_id: &str,
  role: &str,
) -> Result<bool, DbErr> {
  let result = organization_members::Entity::update_many()
    .set(organization_members::ActiveModel {
      role: Set(normalized_member_role(role).to_owned()),
      ..Default::default()
    })
    .filter(organization_members::Column::TenantId.eq(tenant_id))
    .filter(organization_members::Column::OrganizationId.eq(organization_id))
    .filter(organization_members::Column::UserId.eq(user_id))
    .exec(db)
    .await?;
  Ok(result.rows_affected > 0)
}

pub async fn count_members(
  db: &DatabaseConnection,
  tenant_id: &str,
  organization_id: &str,
) -> Result<i64, DbErr> {
  let count = organization_members::Entity::find()
    .filter(organization_members::Column::TenantId.eq(tenant_id))
    .filter(organization_members::Column::OrganizationId.eq(organization_id))
    .count(db)
    .await?;
  Ok(count as i64)
}

/// Owners count, used to refuse removing/demoting the last owner.
pub async fn count_owners(
  db: &DatabaseConnection,
  tenant_id: &str,
  organization_id: &str,
) -> Result<i64, DbErr> {
  let count = organization_members::Entity::find()
    .filter(organization_members::Column::TenantId.eq(tenant_id))
    .filter(organization_members::Column::OrganizationId.eq(organization_id))
    .filter(organization_members::Column::Role.eq("owner"))
    .count(db)
    .await?;
  Ok(count as i64)
}

pub async fn list_members(
  db: &DatabaseConnection,
  tenant_id: &str,
  organization_id: &str,
) -> Result<Vec<OrganizationMember>, DbErr> {
  organization_members::Entity::find()
    .select_only()
    .column_as(users::Column::Id, "user_id")
    .column(users::Column::Email)
    .column(users::Column::DisplayName)
    .column(organization_members::Column::Role)
    .column(organization_members::Column::CreatedAt)
    .join(JoinType::InnerJoin, organization_members::Relation::Users.def())
    .filter(organization_members::Column::TenantId.eq(tenant_id))
    .filter(organization_members::Column::OrganizationId.eq(organization_id))
    .filter(users::Column::TenantId.eq(tenant_id))
    .order_by_asc(organization_members::Column::CreatedAt)
    .order_by_asc(users::Column::Email)
    .into_model::<OrganizationMember>()
    .all(db)
    .await
}

pub async fn user_role(
  db: &DatabaseConnection,
  tenant_id: &str,
  organization_id: &str,
  user_id: &str,
) -> Result<Option<String>, DbErr> {
  Ok(
    organization_members::Entity::find()
      .filter(organization_members::Column::TenantId.eq(tenant_id))
      .filter(organization_members::Column::OrganizationId.eq(organization_id))
      .filter(organization_members::Column::UserId.eq(user_id))
      .one(db)
      .await?
      .map(|m| m.role),
  )
}

pub async fn user_can_access(
  db: &DatabaseConnection,
  tenant_id: &str,
  organization_id: &str,
  user_id: &str,
) -> Result<bool, DbErr> {
  Ok(user_role(db, tenant_id, organization_id, user_id).await?.is_some())
}

/// The member's 0-based seat rank within the org, ordered by join time (then
/// id). Combined with a reported seat limit this decides who is within it:
/// `rank < limit`. Non-members rank past the end.
pub async fn member_seat_rank(
  db: &DatabaseConnection,
  tenant_id: &str,
  organization_id: &str,
  user_id: &str,
) -> Result<i64, DbErr> {
  let me = organization_members::Entity::find()
    .filter(organization_members::Column::TenantId.eq(tenant_id))
    .filter(organization_members::Column::OrganizationId.eq(organization_id))
    .filter(organization_members::Column::UserId.eq(user_id))
    .one(db)
    .await?;
  let Some(me) = me else {
    return Ok(i64::MAX);
  };
  let count = organization_members::Entity::find()
    .filter(organization_members::Column::TenantId.eq(tenant_id))
    .filter(organization_members::Column::OrganizationId.eq(organization_id))
    .filter(
      Condition::any()
        .add(organization_members::Column::CreatedAt.lt(me.created_at))
        .add(
          Condition::all()
            .add(organization_members::Column::CreatedAt.eq(me.created_at))
            .add(organization_members::Column::Id.lt(me.id)),
        ),
    )
    .count(db)
    .await?;
  Ok(count as i64)
}

/// Whether the user holds a seat in any organization (membership in at least
/// one), used to admit org-seat-only users through the sync entitlement gate.
pub async fn has_membership(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
) -> Result<bool, DbErr> {
  let count = organization_members::Entity::find()
    .filter(organization_members::Column::TenantId.eq(tenant_id))
    .filter(organization_members::Column::UserId.eq(user_id))
    .count(db)
    .await?;
  Ok(count > 0)
}
