//! Device counts, and the rank that decides which of an organization's devices
//! fall inside a reported device cap.

use sea_orm::sea_query::{Asterisk, Expr, IntoCondition};
use sea_orm::*;

use crate::entity::{devices, organization_members};

#[derive(FromQueryResult)]
struct Count {
  count: i64,
}

async fn count_rows(
  db: &DatabaseConnection,
  select: Select<devices::Entity>,
) -> Result<i64, DbErr> {
  Ok(
    select
      .select_only()
      .column_as(Expr::col(Asterisk).count(), "count")
      .into_model::<Count>()
      .one(db)
      .await?
      .map_or(0, |row| row.count),
  )
}

/// The org's active devices, reached through its membership rows. There is no
/// devices -> `organization_members` foreign key, so the join is spelled out;
/// pairing on `tenant_id` as well keeps it inside one tenant.
fn active_org_devices(
  tenant_id: &str,
  organization_id: &str,
) -> Select<devices::Entity> {
  let members = devices::Entity::belongs_to(organization_members::Entity)
    .from(devices::Column::UserId)
    .to(organization_members::Column::UserId)
    .on_condition(|left, right| {
      Expr::col((left, devices::Column::TenantId))
        .eq(Expr::col((right, organization_members::Column::TenantId)))
        .into_condition()
    })
    .into();
  devices::Entity::find()
    .join(JoinType::InnerJoin, members)
    .filter(organization_members::Column::TenantId.eq(tenant_id))
    .filter(organization_members::Column::OrganizationId.eq(organization_id))
    .filter(devices::Column::Revoked.eq(0))
}

pub async fn count_for_user(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
) -> Result<i64, DbErr> {
  count_rows(
    db,
    devices::Entity::find()
      .filter(devices::Column::TenantId.eq(tenant_id))
      .filter(devices::Column::UserId.eq(user_id))
      .filter(devices::Column::Revoked.eq(0)),
  )
  .await
}

/// Total active (non-revoked) devices across an organization's members — the
/// org's device footprint against a reported total device limit.
pub async fn count_for_org(
  db: &DatabaseConnection,
  tenant_id: &str,
  organization_id: &str,
) -> Result<i64, DbErr> {
  count_rows(db, active_org_devices(tenant_id, organization_id)).await
}

/// The device's 0-based rank among the org's active devices, ordered by
/// registration time (then id). Combined with a reported device limit this
/// decides which devices are within it: `rank < limit`.
pub async fn device_org_rank(
  db: &DatabaseConnection,
  tenant_id: &str,
  organization_id: &str,
  device_id: &str,
) -> Result<i64, DbErr> {
  let me = devices::Entity::find()
    .filter(devices::Column::TenantId.eq(tenant_id))
    .filter(devices::Column::Id.eq(device_id))
    .one(db)
    .await?;
  let Some(me) = me else {
    return Ok(i64::MAX);
  };
  // Rank is "how many active devices sort before me", so the ordering lives in
  // this predicate rather than an ORDER BY.
  count_rows(
    db,
    active_org_devices(tenant_id, organization_id).filter(
      Condition::any().add(devices::Column::CreatedAt.lt(me.created_at)).add(
        Condition::all()
          .add(devices::Column::CreatedAt.eq(me.created_at))
          .add(devices::Column::Id.lt(me.id)),
      ),
    ),
  )
  .await
}
