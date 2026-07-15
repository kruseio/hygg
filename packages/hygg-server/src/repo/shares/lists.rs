//! Outbox/inbox list queries and the directional active-share counts.

use sea_orm::sea_query::{Expr, IntoCondition, SimpleExpr};
use sea_orm::*;

use super::{ACCEPTED, PENDING, REVOKED};
use crate::entity::{books, document_shares, users};

/// An accepted incoming share, used to badge the recipient's library entry
/// with who shared the document and at what access level.
#[derive(sea_orm::FromQueryResult, Debug, Clone)]
pub struct IncomingShare {
  pub content_hash: String,
  pub from_email: String,
  pub access: String,
}

/// Every document currently shared *to* this user (accepted), with the sharer's
/// email — used to annotate the recipient's home-library cards.
pub async fn accepted_incoming(
  db: &DatabaseConnection,
  tenant_id: &str,
  to_user_id: &str,
) -> Result<Vec<IncomingShare>, DbErr> {
  document_shares::Entity::find()
    .select_only()
    .column(document_shares::Column::ContentHash)
    .column_as(users::Column::Email, "from_email")
    .column(document_shares::Column::Access)
    .join(JoinType::InnerJoin, document_shares::Relation::Users1.def())
    .filter(document_shares::Column::TenantId.eq(tenant_id))
    .filter(document_shares::Column::ToUserId.eq(to_user_id))
    .filter(document_shares::Column::Status.eq(ACCEPTED))
    .into_model::<IncomingShare>()
    .all(db)
    .await
}

/// A share joined with the document's metadata and the counterparty's email
/// (the recipient for an outbox row, the sender for an inbox row).
#[derive(sea_orm::FromQueryResult, Debug, Clone)]
pub struct ShareRow {
  pub id: String,
  pub content_hash: String,
  pub counterparty_email: String,
  pub title: String,
  pub author: String,
  pub format: String,
  pub access: String,
  pub status: String,
  pub created_at: i64,
}

/// Active outgoing shares for the sender (against their cap): pending+accepted.
pub async fn outgoing_active_count(
  db: &DatabaseConnection,
  tenant_id: &str,
  from_user_id: &str,
) -> Result<i64, DbErr> {
  count(
    db,
    tenant_id,
    document_shares::Column::FromUserId,
    from_user_id,
    &[PENDING, ACCEPTED],
  )
  .await
}

/// Active incoming shares for the recipient (against their cap): accepted only.
pub async fn incoming_accepted_count(
  db: &DatabaseConnection,
  tenant_id: &str,
  to_user_id: &str,
) -> Result<i64, DbErr> {
  count(
    db,
    tenant_id,
    document_shares::Column::ToUserId,
    to_user_id,
    &[ACCEPTED],
  )
  .await
}

/// Pending incoming shares awaiting the recipient's decision (inbox badge).
pub async fn pending_inbox_count(
  db: &DatabaseConnection,
  tenant_id: &str,
  to_user_id: &str,
) -> Result<i64, DbErr> {
  count(
    db,
    tenant_id,
    document_shares::Column::ToUserId,
    to_user_id,
    &[PENDING],
  )
  .await
}

async fn count(
  db: &DatabaseConnection,
  tenant_id: &str,
  actor_col: document_shares::Column,
  actor_id: &str,
  statuses: &[&str],
) -> Result<i64, DbErr> {
  let count = document_shares::Entity::find()
    .filter(document_shares::Column::TenantId.eq(tenant_id))
    .filter(actor_col.eq(actor_id))
    .filter(document_shares::Column::Status.is_in(statuses.iter().copied()))
    .count(db)
    .await?;
  Ok(count as i64)
}

/// Outbox: everything the sender has shared (recipient email + doc metadata),
/// newest first, excluding revoked rows.
pub async fn list_outbox(
  db: &DatabaseConnection,
  tenant_id: &str,
  from_user_id: &str,
) -> Result<Vec<ShareRow>, DbErr> {
  list(db, tenant_id, from_user_id, true).await
}

/// Inbox: pending shares awaiting the recipient (sender email + doc metadata).
pub async fn list_inbox(
  db: &DatabaseConnection,
  tenant_id: &str,
  to_user_id: &str,
) -> Result<Vec<ShareRow>, DbErr> {
  list(db, tenant_id, to_user_id, false).await
}

async fn list(
  db: &DatabaseConnection,
  tenant_id: &str,
  actor_id: &str,
  outbox: bool,
) -> Result<Vec<ShareRow>, DbErr> {
  // Outbox shows all non-revoked history and names the recipient; inbox shows
  // only pending shares and names the sender.
  let (user_rel, actor_col, status_pred) = if outbox {
    (
      document_shares::Relation::Users2.def(),
      document_shares::Column::FromUserId,
      document_shares::Column::Status.ne(REVOKED),
    )
  } else {
    (
      document_shares::Relation::Users1.def(),
      document_shares::Column::ToUserId,
      document_shares::Column::Status.eq(PENDING),
    )
  };
  document_shares::Entity::find()
    .select_only()
    .column(document_shares::Column::Id)
    .column(document_shares::Column::ContentHash)
    .column_as(users::Column::Email, "counterparty_email")
    .expr_as(book_text(books::Column::Title), "title")
    .expr_as(book_text(books::Column::Author), "author")
    .expr_as(book_text(books::Column::Format), "format")
    .column(document_shares::Column::Access)
    .column(document_shares::Column::Status)
    .column(document_shares::Column::CreatedAt)
    .join(JoinType::InnerJoin, user_rel)
    .join(JoinType::LeftJoin, books_join())
    .filter(document_shares::Column::TenantId.eq(tenant_id))
    .filter(actor_col.eq(actor_id))
    .filter(status_pred)
    .order_by_desc(document_shares::Column::CreatedAt)
    .into_model::<ShareRow>()
    .all(db)
    .await
}

/// A book column read through the LEFT JOIN: absent when the document was never
/// uploaded here, which the list renders as an empty string.
fn book_text(column: books::Column) -> SimpleExpr {
  Expr::col((books::Entity, column)).if_null("")
}

/// The book carrying a share's content hash. Matched within the tenant, since
/// `books` is unique per `(tenant_id, content_hash)` — not per hash alone.
fn books_join() -> RelationDef {
  document_shares::Entity::belongs_to(books::Entity)
    .from(document_shares::Column::ContentHash)
    .to(books::Column::ContentHash)
    .on_condition(|left, right| {
      Expr::col((left, document_shares::Column::TenantId))
        .equals((right, books::Column::TenantId))
        .into_condition()
    })
    .into()
}

/// Remove every share of a document (called when its owner deletes it).
pub async fn delete_for_hash(
  db: &DatabaseConnection,
  tenant_id: &str,
  content_hash: &str,
) -> Result<(), DbErr> {
  document_shares::Entity::delete_many()
    .filter(document_shares::Column::TenantId.eq(tenant_id))
    .filter(document_shares::Column::ContentHash.eq(content_hash))
    .exec(db)
    .await?;
  Ok(())
}
