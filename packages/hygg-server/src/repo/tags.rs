//! User- and organization-scoped document tags. Personal documents take
//! `user` tags (private to the owner); organization documents take `org` tags
//! shared across the org's members.

use sea_orm::sea_query::{Expr, IntoCondition, OnConflict, Query};
use sea_orm::*;
use serde::Serialize;

use crate::entity::{book_tags, tags};
use crate::util::{new_id, now_millis};

#[derive(FromQueryResult, Serialize, Clone, Debug)]
pub struct BookTag {
  pub content_hash: String,
  pub name: String,
}

/// `book_tags -> tags`, re-scoped to the tenant. The generated relation joins
/// on `tag_id` alone; a tag must never resolve across a tenant boundary.
fn tag_join() -> RelationDef {
  book_tags::Relation::Tags.def().on_condition(|left, right| {
    Expr::col((right, tags::Column::TenantId))
      .equals((left, book_tags::Column::TenantId))
      .into_condition()
  })
}

/// Create the tag if absent and return its id.
pub async fn ensure(
  db: &DatabaseConnection,
  tenant_id: &str,
  scope_type: &str,
  scope_id: &str,
  name: &str,
) -> Result<String, DbErr> {
  tags::Entity::insert(tags::ActiveModel {
    id: Set(new_id()),
    tenant_id: Set(tenant_id.to_owned()),
    scope_type: Set(scope_type.to_owned()),
    scope_id: Set(scope_id.to_owned()),
    name: Set(name.to_owned()),
    created_at: Set(now_millis()),
  })
  .on_conflict(
    OnConflict::columns([
      tags::Column::TenantId,
      tags::Column::ScopeType,
      tags::Column::ScopeId,
      tags::Column::Name,
    ])
    .do_nothing()
    .to_owned(),
  )
  .exec_without_returning(db)
  .await?;
  // Re-read rather than trust the insert: on conflict the pre-existing row's
  // id is the one callers must attach to.
  let id: Option<String> = tags::Entity::find()
    .select_only()
    .column(tags::Column::Id)
    .filter(tags::Column::TenantId.eq(tenant_id))
    .filter(tags::Column::ScopeType.eq(scope_type))
    .filter(tags::Column::ScopeId.eq(scope_id))
    .filter(tags::Column::Name.eq(name))
    .into_tuple()
    .one(db)
    .await?;
  id.ok_or_else(|| DbErr::RecordNotFound("tags".to_owned()))
}

pub async fn attach(
  db: &DatabaseConnection,
  tenant_id: &str,
  tag_id: &str,
  content_hash: &str,
) -> Result<(), DbErr> {
  book_tags::Entity::insert(book_tags::ActiveModel {
    id: Set(new_id()),
    tenant_id: Set(tenant_id.to_owned()),
    tag_id: Set(tag_id.to_owned()),
    content_hash: Set(content_hash.to_owned()),
    created_at: Set(now_millis()),
  })
  .on_conflict(
    OnConflict::columns([
      book_tags::Column::TenantId,
      book_tags::Column::TagId,
      book_tags::Column::ContentHash,
    ])
    .do_nothing()
    .to_owned(),
  )
  .exec_without_returning(db)
  .await?;
  Ok(())
}

pub async fn detach_by_name(
  db: &DatabaseConnection,
  tenant_id: &str,
  scope_type: &str,
  scope_id: &str,
  name: &str,
  content_hash: &str,
) -> Result<bool, DbErr> {
  let result = book_tags::Entity::delete_many()
    .filter(book_tags::Column::TenantId.eq(tenant_id))
    .filter(book_tags::Column::ContentHash.eq(content_hash))
    .filter(
      book_tags::Column::TagId.in_subquery(
        Query::select()
          .column((tags::Entity, tags::Column::Id))
          .from(tags::Entity)
          .and_where(tags::Column::TenantId.eq(tenant_id))
          .and_where(tags::Column::ScopeType.eq(scope_type))
          .and_where(tags::Column::ScopeId.eq(scope_id))
          .and_where(tags::Column::Name.eq(name))
          .to_owned(),
      ),
    )
    .exec(db)
    .await?;
  Ok(result.rows_affected > 0)
}

/// Every `(content_hash, tag name)` in one tag scope.
async fn tags_in_scope(
  db: &DatabaseConnection,
  tenant_id: &str,
  scope_type: &str,
  scope_id: &str,
) -> Result<Vec<BookTag>, DbErr> {
  book_tags::Entity::find()
    .select_only()
    .column(book_tags::Column::ContentHash)
    .column(tags::Column::Name)
    .join(JoinType::InnerJoin, tag_join())
    .filter(book_tags::Column::TenantId.eq(tenant_id))
    .filter(tags::Column::ScopeType.eq(scope_type))
    .filter(tags::Column::ScopeId.eq(scope_id))
    .into_model::<BookTag>()
    .all(db)
    .await
}

/// Every `(content_hash, tag name)` visible to the user: their own `user` tags
/// plus `org` tags for each of the given organizations.
pub async fn visible_book_tags(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
  org_ids: &[String],
) -> Result<Vec<BookTag>, DbErr> {
  let mut out = tags_in_scope(db, tenant_id, "user", user_id).await?;
  for org_id in org_ids {
    out.extend(tags_in_scope(db, tenant_id, "org", org_id).await?);
  }
  Ok(out)
}
