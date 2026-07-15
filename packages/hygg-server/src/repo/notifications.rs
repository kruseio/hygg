//! Persisted, dismissible per-user notifications (limit warnings). Each
//! distinct condition is keyed by `dedupe_key` so it is raised at most once per
//! user until dismissed; a more severe condition uses a different key.

use sea_orm::sea_query::{Expr, OnConflict};
use sea_orm::*;
use serde::Serialize;

use crate::entity::notifications;
use crate::util::{new_id, now_millis};

#[derive(FromQueryResult, Serialize, Clone, Debug)]
pub struct NotificationRow {
  pub id: String,
  pub severity: String,
  pub title: String,
  pub body: String,
  pub created_at: i64,
}

/// Raise a notification for a user. Idempotent on `(tenant, user, dedupe_key)`:
/// an existing row (even dismissed) is left untouched, so a condition isn't
/// re-raised after it's been acknowledged.
#[allow(clippy::too_many_arguments)]
pub async fn upsert(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
  dedupe_key: &str,
  severity: &str,
  title: &str,
  body: &str,
) -> Result<(), DbErr> {
  notifications::Entity::insert(notifications::ActiveModel {
    id: Set(new_id()),
    tenant_id: Set(tenant_id.to_owned()),
    user_id: Set(user_id.to_owned()),
    dedupe_key: Set(dedupe_key.to_owned()),
    severity: Set(severity.to_owned()),
    title: Set(title.to_owned()),
    body: Set(body.to_owned()),
    created_at: Set(now_millis()),
    dismissed_at: NotSet,
  })
  .on_conflict(
    OnConflict::columns([
      notifications::Column::TenantId,
      notifications::Column::UserId,
      notifications::Column::DedupeKey,
    ])
    .do_nothing()
    .to_owned(),
  )
  // A conflict is the expected steady state, not an error, so take the rows
  // affected rather than the `RecordNotInserted` that `exec` would raise.
  .exec_without_returning(db)
  .await?;
  Ok(())
}

pub async fn list_undismissed(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
) -> Result<Vec<NotificationRow>, DbErr> {
  notifications::Entity::find()
    .select_only()
    .columns([
      notifications::Column::Id,
      notifications::Column::Severity,
      notifications::Column::Title,
      notifications::Column::Body,
      notifications::Column::CreatedAt,
    ])
    .filter(notifications::Column::TenantId.eq(tenant_id))
    .filter(notifications::Column::UserId.eq(user_id))
    .filter(notifications::Column::DismissedAt.is_null())
    .order_by_desc(notifications::Column::CreatedAt)
    .into_model::<NotificationRow>()
    .all(db)
    .await
}

pub async fn dismiss(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
  id: &str,
) -> Result<bool, DbErr> {
  let result = notifications::Entity::update_many()
    .col_expr(notifications::Column::DismissedAt, Expr::value(now_millis()))
    .filter(notifications::Column::TenantId.eq(tenant_id))
    .filter(notifications::Column::UserId.eq(user_id))
    .filter(notifications::Column::Id.eq(id))
    .filter(notifications::Column::DismissedAt.is_null())
    .exec(db)
    .await?;
  Ok(result.rows_affected > 0)
}
