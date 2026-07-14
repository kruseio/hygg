use sea_orm::sea_query::{Alias, Asterisk, Expr, SimpleExpr};
use sea_orm::*;

use super::types::BreakdownRow;
use crate::entity::{sessions, users};

pub(super) async fn role_breakdown(
  db: &DatabaseConnection,
  tenant_id: &str,
) -> Result<Vec<BreakdownRow>, DbErr> {
  let label: SimpleExpr =
    Expr::case(users::Column::Role.eq("admin"), "Admin").finally("User").into();
  // Grouped and ordered by the projected aliases rather than the CASE, so the
  // two buckets collapse the way the hand-written query's `GROUP BY label` did.
  users::Entity::find()
    .select_only()
    .column_as(label, "label")
    .column_as(Expr::col(Asterisk).count(), "count")
    .filter(users::Column::TenantId.eq(tenant_id))
    .group_by(Expr::col(Alias::new("label")))
    .order_by(Expr::col(Alias::new("count")), Order::Desc)
    .order_by(Expr::col(Alias::new("label")), Order::Asc)
    .into_model::<BreakdownRow>()
    .all(db)
    .await
}

#[derive(FromQueryResult)]
struct AgentCount {
  user_agent: Option<String>,
  count: i64,
}

pub(super) async fn client_os(
  db: &DatabaseConnection,
  tenant_id: &str,
  since: i64,
) -> Result<Vec<BreakdownRow>, DbErr> {
  let rows = sessions::Entity::find()
    .select_only()
    .column_as(sessions::Column::UserAgent, "user_agent")
    .column_as(Expr::col(Asterisk).count(), "count")
    .filter(sessions::Column::TenantId.eq(tenant_id))
    .filter(sessions::Column::CreatedAt.gte(since))
    .group_by(sessions::Column::UserAgent)
    .into_model::<AgentCount>()
    .all(db)
    .await?;
  let mut counts = std::collections::BTreeMap::<String, i64>::new();
  for row in rows {
    *counts
      .entry(os_label(row.user_agent.as_deref()).to_string())
      .or_default() += row.count;
  }
  Ok(
    counts
      .into_iter()
      .map(|(label, count)| BreakdownRow { label, count })
      .collect(),
  )
}

fn os_label(user_agent: Option<&str>) -> &'static str {
  let agent = user_agent.unwrap_or("").to_lowercase();
  if agent.contains("windows") {
    "Windows"
  } else if agent.contains("iphone")
    || agent.contains("ipad")
    || agent.contains("ios")
  {
    "iOS"
  } else if agent.contains("android") {
    "Android"
  } else if agent.contains("mac os") || agent.contains("macintosh") {
    "macOS"
  } else if agent.contains("linux") {
    "Linux"
  } else {
    "Other"
  }
}
