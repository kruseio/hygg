use sea_orm::sea_query::{
  Asterisk, Expr, Query, SimpleExpr, SubQueryStatement,
};
use sea_orm::*;

use super::{
  OrganizationListItem, OrganizationMembership, OrganizationRow,
  normalized_slug,
};
use crate::entity::{books, organization_members, organizations};
use crate::util::{new_id, now_millis};

pub async fn create(
  db: &DatabaseConnection,
  tenant_id: &str,
  name: &str,
  created_by_user_id: &str,
) -> Result<String, DbErr> {
  let id = new_id();
  let now = now_millis();
  // `default_access` stays unset so the column default decides it.
  organizations::Entity::insert(organizations::ActiveModel {
    id: Set(id.clone()),
    tenant_id: Set(tenant_id.to_owned()),
    name: Set(name.trim().to_owned()),
    slug: Set(normalized_slug(name)),
    created_by_user_id: Set(Some(created_by_user_id.to_owned())),
    created_at: Set(now),
    updated_at: Set(now),
    default_access: NotSet,
  })
  .exec(db)
  .await?;
  super::add_member(db, tenant_id, &id, created_by_user_id, "owner").await?;
  Ok(id)
}

pub async fn find_by_id(
  db: &DatabaseConnection,
  tenant_id: &str,
  organization_id: &str,
) -> Result<Option<OrganizationRow>, DbErr> {
  Ok(
    organizations::Entity::find()
      .filter(organizations::Column::TenantId.eq(tenant_id))
      .filter(organizations::Column::Id.eq(organization_id))
      .one(db)
      .await?
      .map(OrganizationRow::from),
  )
}

pub async fn list_for_user(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
) -> Result<Vec<OrganizationMembership>, DbErr> {
  organizations::Entity::find()
    .select_only()
    .column(organizations::Column::Id)
    .column(organizations::Column::Name)
    .column(organizations::Column::Slug)
    .column(organization_members::Column::Role)
    .join(
      JoinType::InnerJoin,
      organizations::Relation::OrganizationMembers.def(),
    )
    .filter(organizations::Column::TenantId.eq(tenant_id))
    .filter(organization_members::Column::TenantId.eq(tenant_id))
    .filter(organization_members::Column::UserId.eq(user_id))
    .order_by_asc(organizations::Column::Name)
    .into_model::<OrganizationMembership>()
    .all(db)
    .await
}

/// Every organization in the tenant (admin view) with its member count.
pub async fn list_for_tenant(
  db: &DatabaseConnection,
  tenant_id: &str,
) -> Result<Vec<OrganizationListItem>, DbErr> {
  organizations::Entity::find()
    .select_only()
    .column(organizations::Column::Id)
    .column(organizations::Column::Name)
    .column(organizations::Column::Slug)
    .column(organizations::Column::DefaultAccess)
    .expr_as(member_count_subquery(), "member_count")
    .filter(organizations::Column::TenantId.eq(tenant_id))
    .order_by_asc(organizations::Column::Name)
    .into_model::<OrganizationListItem>()
    .all(db)
    .await
}

/// Correlated `COUNT(*)` of the members of the outer `organizations` row.
fn member_count_subquery() -> SimpleExpr {
  SimpleExpr::SubQuery(
    None,
    Box::new(SubQueryStatement::SelectStatement(
      Query::select()
        .expr(Expr::col(Asterisk).count())
        .from(organization_members::Entity)
        .and_where(
          Expr::col((
            organization_members::Entity,
            organization_members::Column::TenantId,
          ))
          .equals((organizations::Entity, organizations::Column::TenantId)),
        )
        .and_where(
          Expr::col((
            organization_members::Entity,
            organization_members::Column::OrganizationId,
          ))
          .equals((organizations::Entity, organizations::Column::Id)),
        )
        .to_owned(),
    )),
  )
}

pub async fn rename(
  db: &DatabaseConnection,
  tenant_id: &str,
  organization_id: &str,
  name: &str,
) -> Result<bool, DbErr> {
  let result = organizations::Entity::update_many()
    .set(organizations::ActiveModel {
      name: Set(name.trim().to_owned()),
      slug: Set(normalized_slug(name)),
      updated_at: Set(now_millis()),
      ..Default::default()
    })
    .filter(organizations::Column::TenantId.eq(tenant_id))
    .filter(organizations::Column::Id.eq(organization_id))
    .exec(db)
    .await?;
  Ok(result.rows_affected > 0)
}

/// Delete an organization. Member rows cascade; `books.organization_id` has no
/// FK, so detach those documents first (back to private) to avoid dangling
/// pointers.
pub async fn delete(
  db: &DatabaseConnection,
  tenant_id: &str,
  organization_id: &str,
) -> Result<bool, DbErr> {
  books::Entity::update_many()
    .set(books::ActiveModel {
      organization_id: Set(None),
      ..Default::default()
    })
    .filter(books::Column::TenantId.eq(tenant_id))
    .filter(books::Column::OrganizationId.eq(organization_id))
    .exec(db)
    .await?;
  let result = organizations::Entity::delete_many()
    .filter(organizations::Column::TenantId.eq(tenant_id))
    .filter(organizations::Column::Id.eq(organization_id))
    .exec(db)
    .await?;
  Ok(result.rows_affected > 0)
}
