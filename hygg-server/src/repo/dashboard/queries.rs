//! Aggregate helpers shared by the dashboard panels. Each takes an already
//! filtered `Select` and projects a single scalar over it.

use sea_orm::sea_query::{Asterisk, Expr, Func, SimpleExpr};
use sea_orm::*;

#[derive(FromQueryResult)]
struct Count {
  count: i64,
}

#[derive(FromQueryResult)]
struct Sum {
  sum: Option<i64>,
}

pub(super) async fn count<E>(
  db: &DatabaseConnection,
  select: Select<E>,
) -> Result<i64, DbErr>
where
  E: EntityTrait,
{
  Ok(
    select
      .select_only()
      .column_as(Expr::col(Asterisk).count(), "count")
      .into_model::<Count>()
      .one(db)
      .await?
      .map_or(0, |row| row.count),
  )
}

pub(super) async fn count_distinct<E>(
  db: &DatabaseConnection,
  select: Select<E>,
  column: E::Column,
) -> Result<i64, DbErr>
where
  E: EntityTrait,
{
  Ok(
    select
      .select_only()
      .column_as(count_distinct_expr(column), "count")
      .into_model::<Count>()
      .one(db)
      .await?
      .map_or(0, |row| row.count),
  )
}

pub(super) fn count_distinct_expr<C>(column: C) -> SimpleExpr
where
  C: ColumnTrait,
{
  Func::count_distinct(column.into_simple_expr()).into()
}

/// `SUM` over no rows is NULL, not 0 — callers report a zero total instead,
/// which is the `COALESCE` the hand-written queries relied on.
pub(super) async fn sum<E>(
  db: &DatabaseConnection,
  select: Select<E>,
  value: SimpleExpr,
) -> Result<i64, DbErr>
where
  E: EntityTrait,
{
  let total: SimpleExpr = Func::sum(value).into();
  Ok(
    select
      .select_only()
      .column_as(total, "sum")
      .into_model::<Sum>()
      .one(db)
      .await?
      .and_then(|row| row.sum)
      .unwrap_or(0),
  )
}
