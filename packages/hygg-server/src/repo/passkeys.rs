//! Passkey rows. The WebAuthn ceremony lives in the web layer; this repository
//! owns durable credential storage and revocation.

use sea_orm::sea_query::Expr;
use sea_orm::*;
use serde::Serialize;

use crate::entity::passkeys;
use crate::util::{new_id, now_millis};

#[derive(FromQueryResult, Serialize, Clone, Debug)]
pub struct PasskeySummary {
  pub id: String,
  pub credential_id: String,
  pub label: String,
  pub disabled: i64,
  pub created_at: i64,
  pub last_used_at: Option<i64>,
}

#[derive(FromQueryResult, Clone, Debug)]
pub struct PasskeyRow {
  pub id: String,
  pub credential_id: String,
  pub passkey_json: String,
  pub label: String,
  pub disabled: i64,
}

pub async fn list_for_user(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
) -> Result<Vec<PasskeySummary>, DbErr> {
  passkeys::Entity::find()
    .select_only()
    .column(passkeys::Column::Id)
    .column(passkeys::Column::CredentialId)
    .column(passkeys::Column::Label)
    .column(passkeys::Column::Disabled)
    .column(passkeys::Column::CreatedAt)
    .column(passkeys::Column::LastUsedAt)
    .filter(passkeys::Column::TenantId.eq(tenant_id))
    .filter(passkeys::Column::UserId.eq(user_id))
    .order_by_desc(passkeys::Column::CreatedAt)
    .into_model::<PasskeySummary>()
    .all(db)
    .await
}

pub async fn list_active_for_user(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
) -> Result<Vec<PasskeyRow>, DbErr> {
  passkeys::Entity::find()
    .select_only()
    .column(passkeys::Column::Id)
    .column(passkeys::Column::CredentialId)
    .column(passkeys::Column::PasskeyJson)
    .column(passkeys::Column::Label)
    .column(passkeys::Column::Disabled)
    .filter(passkeys::Column::TenantId.eq(tenant_id))
    .filter(passkeys::Column::UserId.eq(user_id))
    .filter(passkeys::Column::Disabled.eq(0))
    .order_by_desc(passkeys::Column::CreatedAt)
    .into_model::<PasskeyRow>()
    .all(db)
    .await
}

pub async fn credential_exists(
  db: &DatabaseConnection,
  tenant_id: &str,
  credential_id: &str,
) -> Result<bool, DbErr> {
  let count = passkeys::Entity::find()
    .filter(passkeys::Column::TenantId.eq(tenant_id))
    .filter(passkeys::Column::CredentialId.eq(credential_id))
    .count(db)
    .await?;
  Ok(count > 0)
}

pub async fn insert(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
  credential_id: &str,
  passkey_json: &str,
  label: &str,
) -> Result<String, DbErr> {
  let id = new_id();
  passkeys::ActiveModel {
    id: Set(id.clone()),
    tenant_id: Set(tenant_id.to_owned()),
    user_id: Set(user_id.to_owned()),
    credential_id: Set(credential_id.to_owned()),
    passkey_json: Set(passkey_json.to_owned()),
    label: Set(label.to_owned()),
    disabled: Set(0),
    created_at: Set(now_millis()),
    last_used_at: Set(None),
  }
  .insert(db)
  .await?;
  Ok(id)
}

pub async fn update_after_authentication(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
  passkey_id: &str,
  passkey_json: &str,
) -> Result<bool, DbErr> {
  let result = passkeys::Entity::update_many()
    .col_expr(passkeys::Column::PasskeyJson, Expr::value(passkey_json))
    .col_expr(passkeys::Column::LastUsedAt, Expr::value(now_millis()))
    .filter(passkeys::Column::TenantId.eq(tenant_id))
    .filter(passkeys::Column::UserId.eq(user_id))
    .filter(passkeys::Column::Id.eq(passkey_id))
    .filter(passkeys::Column::Disabled.eq(0))
    .exec(db)
    .await?;
  Ok(result.rows_affected > 0)
}

pub async fn revoke(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
  passkey_id: &str,
) -> Result<bool, DbErr> {
  let result = passkeys::Entity::update_many()
    .col_expr(passkeys::Column::Disabled, Expr::value(1_i64))
    .filter(passkeys::Column::TenantId.eq(tenant_id))
    .filter(passkeys::Column::UserId.eq(user_id))
    .filter(passkeys::Column::Id.eq(passkey_id))
    .exec(db)
    .await?;
  Ok(result.rows_affected > 0)
}
