use sea_orm::sea_query::{Asterisk, Expr};
use sea_orm::*;

use super::queries::{count, count_distinct, count_distinct_expr, sum};
use super::types::{ActivityRow, ResourceMetricRow};
use crate::entity::{
  book_blobs, bookmarks, books, highlights, notes, organization_members,
  organizations, progress, sessions,
};

pub(super) async fn activity(
  db: &DatabaseConnection,
  tenant_id: &str,
  since: i64,
) -> Result<Vec<ActivityRow>, DbErr> {
  Ok(vec![
    activity_for(
      db,
      progress::Entity::find()
        .filter(progress::Column::TenantId.eq(tenant_id))
        .filter(progress::Column::UpdatedAt.gte(since)),
      progress::Column::UserId,
      "Progress",
      "Position update",
    )
    .await?,
    activity_for(
      db,
      bookmarks::Entity::find()
        .filter(bookmarks::Column::TenantId.eq(tenant_id))
        .filter(bookmarks::Column::UpdatedAt.gte(since)),
      bookmarks::Column::UserId,
      "Bookmarks",
      "Bookmark change",
    )
    .await?,
    activity_for(
      db,
      highlights::Entity::find()
        .filter(highlights::Column::TenantId.eq(tenant_id))
        .filter(highlights::Column::UpdatedAt.gte(since)),
      highlights::Column::UserId,
      "Highlights",
      "Highlight change",
    )
    .await?,
    activity_for(
      db,
      notes::Entity::find()
        .filter(notes::Column::TenantId.eq(tenant_id))
        .filter(notes::Column::UpdatedAt.gte(since)),
      notes::Column::UserId,
      "Notes",
      "Note change",
    )
    .await?,
  ])
}

#[derive(FromQueryResult)]
struct Counts {
  events: i64,
  users: i64,
}

/// Both counts are projected from one scan, as the single query they replace
/// did — a second pass could see a different set of rows.
async fn activity_for<E>(
  db: &DatabaseConnection,
  select: Select<E>,
  user_id: E::Column,
  label: &str,
  event: &str,
) -> Result<ActivityRow, DbErr>
where
  E: EntityTrait,
{
  let row = select
    .select_only()
    .column_as(Expr::col(Asterisk).count(), "events")
    .column_as(count_distinct_expr(user_id), "users")
    .into_model::<Counts>()
    .one(db)
    .await?;
  Ok(ActivityRow {
    label: label.to_string(),
    event: event.to_string(),
    events: row.as_ref().map_or(0, |row| row.events),
    users: row.map_or(0, |row| row.users),
  })
}

pub(super) async fn resource_metrics(
  db: &DatabaseConnection,
  tenant_id: &str,
  since: i64,
) -> Result<Vec<ResourceMetricRow>, DbErr> {
  let all_books =
    || books::Entity::find().filter(books::Column::TenantId.eq(tenant_id));
  let all_blobs = || {
    book_blobs::Entity::find()
      .filter(book_blobs::Column::TenantId.eq(tenant_id))
  };
  let all_orgs = || {
    organizations::Entity::find()
      .filter(organizations::Column::TenantId.eq(tenant_id))
  };
  let all_members = || {
    organization_members::Entity::find()
      .filter(organization_members::Column::TenantId.eq(tenant_id))
  };
  let all_sessions = || {
    sessions::Entity::find().filter(sessions::Column::TenantId.eq(tenant_id))
  };
  Ok(vec![
    ResourceMetricRow {
      label: "Documents".to_string(),
      total: count(db, all_books()).await?,
      recent: count(
        db,
        all_books().filter(books::Column::CreatedAt.gte(since)),
      )
      .await?,
      actors: count_distinct(db, all_books(), books::Column::OwnerUserId)
        .await?,
      size_bytes: sum(
        db,
        all_books(),
        books::Column::SizeBytes.into_simple_expr(),
      )
      .await?,
    },
    ResourceMetricRow {
      label: "Document blobs".to_string(),
      total: count(db, all_blobs()).await?,
      recent: count(
        db,
        all_blobs().filter(book_blobs::Column::CreatedAt.gte(since)),
      )
      .await?,
      actors: 0,
      size_bytes: sum(
        db,
        all_blobs(),
        book_blobs::Column::ByteLen.into_simple_expr(),
      )
      .await?,
    },
    ResourceMetricRow {
      label: "Organizations".to_string(),
      total: count(db, all_orgs()).await?,
      recent: count(
        db,
        all_orgs().filter(organizations::Column::CreatedAt.gte(since)),
      )
      .await?,
      actors: count_distinct(
        db,
        all_members(),
        organization_members::Column::UserId,
      )
      .await?,
      size_bytes: 0,
    },
    ResourceMetricRow {
      label: "Sessions".to_string(),
      total: count(db, all_sessions()).await?,
      recent: count(
        db,
        all_sessions().filter(sessions::Column::CreatedAt.gte(since)),
      )
      .await?,
      actors: count_distinct(db, all_sessions(), sessions::Column::UserId)
        .await?,
      size_bytes: 0,
    },
  ])
}
