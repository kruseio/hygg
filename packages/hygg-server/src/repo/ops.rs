//! Idempotency ledger. Each applied sync op records its `op_id` here so a
//! batch resent after a reconnect is a no-op (the client retries safely).

use sea_orm::sea_query::OnConflict;
use sea_orm::*;

use crate::entity::applied_ops;
use crate::util::now_millis;

pub async fn was_applied(
  db: &DatabaseConnection,
  tenant_id: &str,
  op_id: &str,
) -> Result<bool, DbErr> {
  let found = applied_ops::Entity::find()
    .filter(applied_ops::Column::TenantId.eq(tenant_id))
    .filter(applied_ops::Column::OpId.eq(op_id))
    .one(db)
    .await?;
  Ok(found.is_some())
}

pub async fn mark_applied(
  db: &DatabaseConnection,
  tenant_id: &str,
  op_id: &str,
) -> Result<(), DbErr> {
  let am = applied_ops::ActiveModel {
    tenant_id: Set(tenant_id.to_owned()),
    op_id: Set(op_id.to_owned()),
    applied_at: Set(now_millis()),
  };
  // Re-marking an op keeps the first `applied_at`, so a retry never looks newer
  // than the apply it repeats.
  applied_ops::Entity::insert(am)
    .on_conflict(
      OnConflict::columns([
        applied_ops::Column::TenantId,
        applied_ops::Column::OpId,
      ])
      .do_nothing()
      .to_owned(),
    )
    .exec_without_returning(db)
    .await?;
  Ok(())
}
