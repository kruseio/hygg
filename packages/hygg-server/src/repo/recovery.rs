//! One-time recovery tokens issued by admins. Tokens are high-entropy random
//! strings, so SHA-256 hashing is sufficient because plaintext is shown once.

use sea_orm::sea_query::Expr;
use sea_orm::*;

use crate::entity::recovery_codes;
use crate::util::{new_id, now_millis};

pub async fn insert(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
  code_hash: &str,
  issued_by: &str,
  expires_at: i64,
) -> Result<String, DbErr> {
  let id = new_id();
  recovery_codes::ActiveModel {
    id: Set(id.clone()),
    tenant_id: Set(tenant_id.to_owned()),
    user_id: Set(user_id.to_owned()),
    code_hash: Set(code_hash.to_owned()),
    issued_by: Set(Some(issued_by.to_owned())),
    created_at: Set(now_millis()),
    expires_at: Set(expires_at),
    used_at: Set(None),
    consumed: Set(0),
  }
  .insert(db)
  .await?;
  Ok(id)
}

/// Consume a matching active recovery token. Returns false for expired, used,
/// or unknown codes.
pub async fn consume_matching(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
  code_hash: &str,
) -> Result<bool, DbErr> {
  let now = now_millis();
  let result = recovery_codes::Entity::update_many()
    .col_expr(recovery_codes::Column::Consumed, Expr::value(1_i64))
    .col_expr(recovery_codes::Column::UsedAt, Expr::value(now))
    .filter(recovery_codes::Column::TenantId.eq(tenant_id))
    .filter(recovery_codes::Column::UserId.eq(user_id))
    .filter(recovery_codes::Column::CodeHash.eq(code_hash))
    .filter(recovery_codes::Column::Consumed.eq(0))
    .filter(recovery_codes::Column::UsedAt.is_null())
    .filter(recovery_codes::Column::ExpiresAt.gt(now))
    .exec(db)
    .await?;
  Ok(result.rows_affected > 0)
}
