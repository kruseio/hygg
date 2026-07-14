use sea_orm::sea_query::{Expr, SimpleExpr};
use sea_orm::*;
use serde::Serialize;

use crate::entity::users;
use crate::util::{new_id, now_millis};

#[derive(FromQueryResult, Clone, Debug)]
pub struct UserRow {
  pub id: String,
  pub tenant_id: String,
  pub email: String,
  pub display_name: String,
  pub password_hash: Option<String>,
  pub password_enabled: i64,
  pub role: String,
  pub disabled: i64,
}

#[derive(FromQueryResult, Serialize, Clone, Debug)]
pub struct UserSummary {
  pub id: String,
  pub email: String,
  pub display_name: String,
  pub password_enabled: i64,
  pub role: String,
  pub disabled: i64,
  pub created_at: i64,
  pub updated_at: i64,
}

/// The identity and authorization facts a caller needs to authenticate a user;
/// the timestamps are left to [`UserSummary`].
fn user_row_select() -> Select<users::Entity> {
  users::Entity::find().select_only().columns([
    users::Column::Id,
    users::Column::TenantId,
    users::Column::Email,
    users::Column::DisplayName,
    users::Column::PasswordHash,
    users::Column::PasswordEnabled,
    users::Column::Role,
    users::Column::Disabled,
  ])
}

pub async fn find_by_email(
  db: &DatabaseConnection,
  tenant_id: &str,
  email: &str,
) -> Result<Option<UserRow>, DbErr> {
  user_row_select()
    .filter(users::Column::TenantId.eq(tenant_id))
    .filter(users::Column::Email.eq(email))
    .into_model::<UserRow>()
    .one(db)
    .await
}

pub async fn find_by_id(
  db: &DatabaseConnection,
  tenant_id: &str,
  id: &str,
) -> Result<Option<UserRow>, DbErr> {
  user_row_select()
    .filter(users::Column::TenantId.eq(tenant_id))
    .filter(users::Column::Id.eq(id))
    .into_model::<UserRow>()
    .one(db)
    .await
}

pub async fn insert(
  db: &DatabaseConnection,
  tenant_id: &str,
  email: &str,
  display_name: &str,
  password_hash: Option<&str>,
  role: &str,
) -> Result<String, DbErr> {
  let id = new_id();
  let now = now_millis();
  let password_enabled = i64::from(password_hash.is_some());
  users::ActiveModel {
    id: Set(id.clone()),
    tenant_id: Set(tenant_id.to_owned()),
    email: Set(email.to_owned()),
    display_name: Set(display_name.to_owned()),
    password_hash: Set(password_hash.map(ToOwned::to_owned)),
    password_enabled: Set(password_enabled),
    role: Set(role.to_owned()),
    disabled: Set(0),
    created_at: Set(now),
    updated_at: Set(now),
  }
  .insert(db)
  .await?;
  Ok(id)
}

pub async fn list_for_tenant(
  db: &DatabaseConnection,
  tenant_id: &str,
) -> Result<Vec<UserSummary>, DbErr> {
  users::Entity::find()
    .select_only()
    .columns([
      users::Column::Id,
      users::Column::Email,
      users::Column::DisplayName,
      users::Column::PasswordEnabled,
      users::Column::Role,
      users::Column::Disabled,
      users::Column::CreatedAt,
      users::Column::UpdatedAt,
    ])
    .filter(users::Column::TenantId.eq(tenant_id))
    .order_by_desc(users::Column::CreatedAt)
    .into_model::<UserSummary>()
    .all(db)
    .await
}

/// Ids of every active admin in the tenant — recipients for server-wide
/// notifications (e.g. server storage warnings).
pub async fn admin_ids(
  db: &DatabaseConnection,
  tenant_id: &str,
) -> Result<Vec<String>, DbErr> {
  users::Entity::find()
    .select_only()
    .column(users::Column::Id)
    .filter(users::Column::TenantId.eq(tenant_id))
    .filter(users::Column::Role.eq("admin"))
    .filter(users::Column::Disabled.eq(0))
    .into_tuple::<String>()
    .all(db)
    .await
}

pub async fn count_admins(
  db: &DatabaseConnection,
  tenant_id: &str,
) -> Result<i64, DbErr> {
  let count = users::Entity::find()
    .filter(users::Column::TenantId.eq(tenant_id))
    .filter(users::Column::Role.eq("admin"))
    .filter(users::Column::Disabled.eq(0))
    .count(db)
    .await?;
  Ok(count as i64)
}

pub async fn set_role(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
  role: &str,
) -> Result<bool, DbErr> {
  update_one(db, tenant_id, user_id, users::Column::Role, Expr::value(role))
    .await
}

pub async fn set_disabled(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
  disabled: bool,
) -> Result<bool, DbErr> {
  update_one(
    db,
    tenant_id,
    user_id,
    users::Column::Disabled,
    Expr::value(i64::from(disabled)),
  )
  .await
}

pub async fn set_password_enabled(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
  enabled: bool,
) -> Result<bool, DbErr> {
  update_one(
    db,
    tenant_id,
    user_id,
    users::Column::PasswordEnabled,
    Expr::value(i64::from(enabled)),
  )
  .await
}

pub async fn set_password_hash(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
  password_hash: &str,
) -> Result<bool, DbErr> {
  let result = users::Entity::update_many()
    .col_expr(users::Column::PasswordHash, Expr::value(password_hash))
    .col_expr(users::Column::PasswordEnabled, Expr::value(1_i64))
    .col_expr(users::Column::UpdatedAt, Expr::value(now_millis()))
    .filter(users::Column::TenantId.eq(tenant_id))
    .filter(users::Column::Id.eq(user_id))
    .exec(db)
    .await?;
  Ok(result.rows_affected > 0)
}

/// Every single-column mutation here also stamps `updated_at` and is scoped to
/// one user in one tenant, so the setters share this shape.
async fn update_one(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
  column: users::Column,
  value: SimpleExpr,
) -> Result<bool, DbErr> {
  let result = users::Entity::update_many()
    .col_expr(column, value)
    .col_expr(users::Column::UpdatedAt, Expr::value(now_millis()))
    .filter(users::Column::TenantId.eq(tenant_id))
    .filter(users::Column::Id.eq(user_id))
    .exec(db)
    .await?;
  Ok(result.rows_affected > 0)
}
