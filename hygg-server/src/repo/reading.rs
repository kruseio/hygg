//! Reading statistics: per-(book, device) cumulative active reading time and
//! per-(device, day) reading seconds (for streaks). Both apply last-write-wins
//! by `updated_at`, mirroring `repo::progress`. Totals sum across a user's
//! devices.

use std::collections::BTreeSet;

use chrono::{Duration, NaiveDate};
use sea_orm::sea_query::{Alias, Expr, OnConflict};
use sea_orm::*;

use crate::entity::{reading_days, reading_time};
use crate::util::new_id;

pub struct ReadingTimeInput {
  pub book_id: String,
  pub device_id: String,
  pub seconds: i64,
  pub op_id: String,
  pub updated_at: i64,
}

pub struct ReadingDayInput {
  pub device_id: String,
  pub day: String,
  pub seconds: i64,
  pub op_id: String,
  pub updated_at: i64,
}

/// Upsert per-(book, device) cumulative reading seconds (last-write-wins).
pub async fn upsert_time(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
  input: &ReadingTimeInput,
) -> Result<(), DbErr> {
  let am = reading_time::ActiveModel {
    id: Set(new_id()),
    tenant_id: Set(tenant_id.to_owned()),
    user_id: Set(user_id.to_owned()),
    book_id: Set(input.book_id.clone()),
    device_id: Set(input.device_id.clone()),
    seconds: Set(input.seconds),
    op_id: Set(Some(input.op_id.clone())),
    updated_at: Set(input.updated_at),
  };
  reading_time::Entity::insert(am)
    .on_conflict(
      OnConflict::columns([
        reading_time::Column::TenantId,
        reading_time::Column::UserId,
        reading_time::Column::BookId,
        reading_time::Column::DeviceId,
      ])
      .update_columns([
        reading_time::Column::Seconds,
        reading_time::Column::OpId,
        reading_time::Column::UpdatedAt,
      ])
      .action_and_where(
        Expr::col((Alias::new("excluded"), reading_time::Column::UpdatedAt))
          .gte(Expr::col((
            reading_time::Entity,
            reading_time::Column::UpdatedAt,
          ))),
      )
      .to_owned(),
    )
    .exec_without_returning(db)
    .await?;
  Ok(())
}

/// Upsert per-(device, day) cumulative reading seconds (last-write-wins).
pub async fn upsert_day(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
  input: &ReadingDayInput,
) -> Result<(), DbErr> {
  let am = reading_days::ActiveModel {
    id: Set(new_id()),
    tenant_id: Set(tenant_id.to_owned()),
    user_id: Set(user_id.to_owned()),
    device_id: Set(input.device_id.clone()),
    day: Set(input.day.clone()),
    seconds: Set(input.seconds),
    op_id: Set(Some(input.op_id.clone())),
    updated_at: Set(input.updated_at),
  };
  reading_days::Entity::insert(am)
    .on_conflict(
      OnConflict::columns([
        reading_days::Column::TenantId,
        reading_days::Column::UserId,
        reading_days::Column::DeviceId,
        reading_days::Column::Day,
      ])
      .update_columns([
        reading_days::Column::Seconds,
        reading_days::Column::OpId,
        reading_days::Column::UpdatedAt,
      ])
      .action_and_where(
        Expr::col((Alias::new("excluded"), reading_days::Column::UpdatedAt))
          .gte(Expr::col((
            reading_days::Entity,
            reading_days::Column::UpdatedAt,
          ))),
      )
      .to_owned(),
    )
    .exec_without_returning(db)
    .await?;
  Ok(())
}

/// Reading seconds per book, summed across the user's devices.
pub async fn seconds_by_book(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
) -> Result<Vec<(String, i64)>, DbErr> {
  reading_time::Entity::find()
    .select_only()
    .column(reading_time::Column::BookId)
    .column_as(reading_time::Column::Seconds.sum(), "seconds")
    .filter(reading_time::Column::TenantId.eq(tenant_id))
    .filter(reading_time::Column::UserId.eq(user_id))
    .group_by(reading_time::Column::BookId)
    .into_tuple::<(String, i64)>()
    .all(db)
    .await
}

/// Total reading seconds across all books and devices for a user.
pub async fn total_seconds(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
) -> Result<i64, DbErr> {
  // SUM over no rows is NULL, which is a zero total rather than a missing row.
  let total: Option<i64> = reading_time::Entity::find()
    .select_only()
    .column_as(reading_time::Column::Seconds.sum(), "total")
    .filter(reading_time::Column::TenantId.eq(tenant_id))
    .filter(reading_time::Column::UserId.eq(user_id))
    .into_tuple::<Option<i64>>()
    .one(db)
    .await?
    .flatten();
  Ok(total.unwrap_or(0))
}

/// Distinct calendar days (`YYYY-MM-DD`) with any reading activity.
pub async fn active_days(
  db: &DatabaseConnection,
  tenant_id: &str,
  user_id: &str,
) -> Result<Vec<String>, DbErr> {
  reading_days::Entity::find()
    .select_only()
    .column(reading_days::Column::Day)
    .distinct()
    .filter(reading_days::Column::TenantId.eq(tenant_id))
    .filter(reading_days::Column::UserId.eq(user_id))
    .order_by_asc(reading_days::Column::Day)
    .into_tuple::<String>()
    .all(db)
    .await
}

/// Consecutive days with reading activity ending today (or yesterday, so a day
/// not yet read does not break the streak). `today` is injected for testing.
pub fn streak_for_days(days: &[String], today: NaiveDate) -> u64 {
  let parsed: BTreeSet<NaiveDate> = days
    .iter()
    .filter_map(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
    .collect();
  let mut cursor = if parsed.contains(&today) {
    today
  } else {
    let yesterday = today - Duration::days(1);
    if parsed.contains(&yesterday) {
      yesterday
    } else {
      return 0;
    }
  };
  let mut streak = 0u64;
  while parsed.contains(&cursor) {
    streak += 1;
    cursor -= Duration::days(1);
  }
  streak
}

#[cfg(test)]
mod tests {
  use super::*;

  fn date(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
  }

  #[test]
  fn streak_counts_consecutive_days() {
    let days = vec![
      "2026-06-23".to_string(),
      "2026-06-24".to_string(),
      "2026-06-25".to_string(),
    ];
    assert_eq!(streak_for_days(&days, date("2026-06-25")), 3);
  }

  #[test]
  fn streak_allows_unread_today() {
    let days = vec!["2026-06-23".to_string(), "2026-06-24".to_string()];
    assert_eq!(streak_for_days(&days, date("2026-06-25")), 2);
  }

  #[test]
  fn streak_breaks_on_gap_and_when_stale() {
    let days = vec!["2026-06-20".to_string(), "2026-06-25".to_string()];
    assert_eq!(streak_for_days(&days, date("2026-06-25")), 1);
    let stale = vec!["2026-06-20".to_string()];
    assert_eq!(streak_for_days(&stale, date("2026-06-25")), 0);
  }
}
