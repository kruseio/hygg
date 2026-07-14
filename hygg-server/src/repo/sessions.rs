//! DB-backed browser sessions for the server-rendered web UI. The cookie holds
//! only a random opaque id; session state and CSRF secret live server-side.

use sea_orm::sea_query::{Expr, Func, IntoCondition, SimpleExpr};
use sea_orm::*;

use crate::entity::{sessions, users};
use crate::util::now_millis;

#[derive(FromQueryResult, Clone, Debug)]
pub struct SessionSummary {
  pub id: String,
  pub created_at: i64,
  pub last_used_at: Option<i64>,
  pub expires_at: i64,
  pub ip: Option<String>,
  pub user_agent: Option<String>,
}

#[derive(FromQueryResult, Clone, Debug)]
pub struct SessionUserRow {
  pub session_id: String,
  pub tenant_id: String,
  pub user_id: String,
  pub email: String,
  pub display_name: String,
  pub role: String,
  pub disabled: i64,
  pub password_enabled: i64,
  pub csrf_secret: String,
  pub expires_at: i64,
  pub last_used_at: Option<i64>,
}

// Mirrors the `sessions` row: nine columns, several of them independent
// scalars (ids, csrf secret, expiry, client ip/user-agent) with no natural
// grouping, so a params struct would just add indirection.
#[allow(clippy::too_many_arguments)]
pub async fn insert(
  db: &DatabaseConnection,
  session_id: &str,
  tenant_id: &str,
  user_id: &str,
  csrf_secret: &str,
  expires_at: i64,
  ip: Option<&str>,
  user_agent: Option<&str>,
) -> Result<(), DbErr> {
  sessions::ActiveModel {
    id: Set(session_id.to_owned()),
    tenant_id: Set(tenant_id.to_owned()),
    user_id: Set(user_id.to_owned()),
    csrf_secret: Set(csrf_secret.to_owned()),
    created_at: Set(now_millis()),
    last_used_at: Set(Some(now_millis())),
    expires_at: Set(expires_at),
    ip: Set(ip.map(ToOwned::to_owned)),
    user_agent: Set(user_agent.map(ToOwned::to_owned)),
  }
  .insert(db)
  .await?;
  Ok(())
}

pub async fn touch(
  db: &DatabaseConnection,
  session_id: &str,
  expires_at: i64,
) -> Result<(), DbErr> {
  sessions::Entity::update_many()
    .col_expr(sessions::Column::LastUsedAt, Expr::value(now_millis()))
    .col_expr(sessions::Column::ExpiresAt, Expr::value(expires_at))
    .filter(sessions::Column::Id.eq(session_id))
    .exec(db)
    .await?;
  Ok(())
}

pub async fn find(
  db: &DatabaseConnection,
  session_id: &str,
) -> Result<Option<SessionUserRow>, DbErr> {
  sessions::Entity::find()
    .select_only()
    .column_as(sessions::Column::Id, "session_id")
    .column(sessions::Column::TenantId)
    .column(sessions::Column::UserId)
    .column(users::Column::Email)
    .column(users::Column::DisplayName)
    .column(users::Column::Role)
    .column(users::Column::Disabled)
    .column(users::Column::PasswordEnabled)
    .column(sessions::Column::CsrfSecret)
    .column(sessions::Column::ExpiresAt)
    .column(sessions::Column::LastUsedAt)
    .join(JoinType::InnerJoin, user_of_same_tenant())
    .filter(sessions::Column::Id.eq(session_id))
    .into_model::<SessionUserRow>()
    .one(db)
    .await
}

pub async fn list_for_user(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
) -> Result<Vec<SessionSummary>, DbErr> {
  sessions::Entity::find()
    .select_only()
    .columns([
      sessions::Column::Id,
      sessions::Column::CreatedAt,
      sessions::Column::LastUsedAt,
      sessions::Column::ExpiresAt,
      sessions::Column::Ip,
      sessions::Column::UserAgent,
    ])
    .filter(sessions::Column::TenantId.eq(tenant_id))
    .filter(sessions::Column::UserId.eq(user_id))
    .filter(sessions::Column::ExpiresAt.gt(now_millis()))
    .order_by_desc(last_activity())
    .into_model::<SessionSummary>()
    .all(db)
    .await
}

pub async fn delete(
  db: &DatabaseConnection,
  session_id: &str,
) -> Result<(), DbErr> {
  sessions::Entity::delete_many()
    .filter(sessions::Column::Id.eq(session_id))
    .exec(db)
    .await?;
  Ok(())
}

pub async fn revoke_for_user(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
  session_id: &str,
) -> Result<bool, DbErr> {
  let result = sessions::Entity::delete_many()
    .filter(sessions::Column::TenantId.eq(tenant_id))
    .filter(sessions::Column::UserId.eq(user_id))
    .filter(sessions::Column::Id.eq(session_id))
    .exec(db)
    .await?;
  Ok(result.rows_affected > 0)
}

pub async fn revoke_all_for_user(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
) -> Result<u64, DbErr> {
  let result = sessions::Entity::delete_many()
    .filter(sessions::Column::TenantId.eq(tenant_id))
    .filter(sessions::Column::UserId.eq(user_id))
    .exec(db)
    .await?;
  Ok(result.rows_affected)
}

pub async fn delete_expired(db: &DatabaseConnection) -> Result<(), DbErr> {
  sessions::Entity::delete_many()
    .filter(sessions::Column::ExpiresAt.lte(now_millis()))
    .exec(db)
    .await?;
  Ok(())
}

/// The generated relation matches on user id alone; tenant equality is added
/// here so a session can never resolve to another tenant's user.
fn user_of_same_tenant() -> RelationDef {
  sessions::Relation::Users.def().on_condition(|left, right| {
    Expr::col((left, sessions::Column::TenantId))
      .eq(Expr::col((right, users::Column::TenantId)))
      .into_condition()
  })
}

/// A session that has not been used since it was created sorts by its creation
/// time.
fn last_activity() -> SimpleExpr {
  SimpleExpr::from(Func::coalesce([
    Expr::col((sessions::Entity, sessions::Column::LastUsedAt)).into(),
    Expr::col((sessions::Entity, sessions::Column::CreatedAt)).into(),
  ]))
}
