use sea_orm::sea_query::{Expr, Func, IntoCondition, SimpleExpr};
use sea_orm::*;

use crate::entity::{api_tokens, devices};
use crate::util::{new_id, now_millis};

#[derive(FromQueryResult, Clone, Debug)]
pub struct TokenRow {
  pub id: String,
  pub tenant_id: String,
  pub device_id: String,
  pub token_hash: String,
  pub revoked: i64,
  pub expires_at: Option<i64>,
}

#[derive(FromQueryResult, Clone, Debug)]
pub struct ApiTokenSession {
  pub id: String,
  pub prefix: String,
  pub device_id: String,
  pub device_name: String,
  pub platform: String,
  pub created_at: i64,
  pub last_used_at: Option<i64>,
  pub expires_at: Option<i64>,
  pub revoked: i64,
  pub device_revoked: i64,
  pub device_last_seen_at: Option<i64>,
}

pub async fn insert(
  db: &DatabaseConnection,
  tenant_id: &str,
  device_id: &str,
  prefix: &str,
  token_hash: &str,
) -> Result<String, DbErr> {
  let id = new_id();
  api_tokens::ActiveModel {
    id: Set(id.clone()),
    tenant_id: Set(tenant_id.to_owned()),
    device_id: Set(device_id.to_owned()),
    prefix: Set(prefix.to_owned()),
    token_hash: Set(token_hash.to_owned()),
    created_at: Set(now_millis()),
    last_used_at: Set(None),
    expires_at: Set(None),
    revoked: Set(0),
  }
  .insert(db)
  .await?;
  Ok(id)
}

/// Look up a token by its public prefix. Prefixes are globally unique, so this
/// is not tenant-scoped; the caller derives the tenant from the returned row.
pub async fn find_by_prefix(
  db: &DatabaseConnection,
  prefix: &str,
) -> Result<Option<TokenRow>, DbErr> {
  api_tokens::Entity::find()
    .select_only()
    .columns([
      api_tokens::Column::Id,
      api_tokens::Column::TenantId,
      api_tokens::Column::DeviceId,
      api_tokens::Column::TokenHash,
      api_tokens::Column::Revoked,
      api_tokens::Column::ExpiresAt,
    ])
    .filter(api_tokens::Column::Prefix.eq(prefix))
    .into_model::<TokenRow>()
    .one(db)
    .await
}

pub async fn list_for_user(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
) -> Result<Vec<ApiTokenSession>, DbErr> {
  api_tokens::Entity::find()
    .select_only()
    .column(api_tokens::Column::Id)
    .column(api_tokens::Column::Prefix)
    .column(api_tokens::Column::DeviceId)
    .column_as(devices::Column::Name, "device_name")
    .column(devices::Column::Platform)
    .column(api_tokens::Column::CreatedAt)
    .column(api_tokens::Column::LastUsedAt)
    .column(api_tokens::Column::ExpiresAt)
    .column_as(api_tokens::Column::Revoked, "revoked")
    .column_as(devices::Column::Revoked, "device_revoked")
    .column_as(devices::Column::LastSeenAt, "device_last_seen_at")
    .join(JoinType::InnerJoin, device_of_same_tenant())
    .filter(api_tokens::Column::TenantId.eq(tenant_id))
    .filter(devices::Column::UserId.eq(user_id))
    .order_by_desc(last_activity())
    .into_model::<ApiTokenSession>()
    .all(db)
    .await
}

/// Revoke every token belonging to a device (called when the device itself is
/// revoked) so cached tokens stop working immediately.
pub async fn revoke_for_device(
  db: &DatabaseConnection,
  tenant_id: &str,
  device_id: &str,
) -> Result<(), DbErr> {
  api_tokens::Entity::update_many()
    .col_expr(api_tokens::Column::Revoked, Expr::value(1_i64))
    .filter(api_tokens::Column::TenantId.eq(tenant_id))
    .filter(api_tokens::Column::DeviceId.eq(device_id))
    .exec(db)
    .await?;
  Ok(())
}

pub async fn touch_last_used(
  db: &DatabaseConnection,
  id: &str,
) -> Result<(), DbErr> {
  api_tokens::Entity::update_many()
    .col_expr(api_tokens::Column::LastUsedAt, Expr::value(now_millis()))
    .filter(api_tokens::Column::Id.eq(id))
    .exec(db)
    .await?;
  Ok(())
}

/// The generated relation matches on device id alone; tenant equality is added
/// here so the join can never reach across tenants.
fn device_of_same_tenant() -> RelationDef {
  api_tokens::Relation::Devices.def().on_condition(|left, right| {
    Expr::col((left, api_tokens::Column::TenantId))
      .eq(Expr::col((right, devices::Column::TenantId)))
      .into_condition()
  })
}

/// A token that has never been used falls back to its device's last contact,
/// and failing that to when the token was minted.
fn last_activity() -> SimpleExpr {
  SimpleExpr::from(Func::coalesce([
    Expr::col((api_tokens::Entity, api_tokens::Column::LastUsedAt)).into(),
    Expr::col((devices::Entity, devices::Column::LastSeenAt)).into(),
    Expr::col((api_tokens::Entity, api_tokens::Column::CreatedAt)).into(),
  ]))
}
