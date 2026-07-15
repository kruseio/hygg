//! Local, device-side reading statistics: per-day active-reading buckets that
//! feed the server-side reading streak, plus the finished-threshold and
//! idle-timeout constants shared with the reader and home dashboard. All state
//! is local — the sync layer mirrors the per-day seconds and per-book time to
//! the server separately.

use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::{self, BufRead, Write};

use chrono::Local;
use serde::{Deserialize, Serialize};

use crate::utils::get_hygg_config_file;

/// Progress at or above this percentage counts a document as "finished".
pub const FINISHED_PERCENTAGE: f64 = 98.0;

/// Idle gap after which reading time stops accruing (no keypress). Reading a
/// dense page without input still counts up to this bound.
pub const READING_IDLE_SECONDS: u64 = 180;

#[derive(Serialize, Deserialize)]
enum DayEvent {
  ReadingDay { day: String, seconds: u64 },
}

fn days_file() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
  get_hygg_config_file(".reading_days.jsonl")
}

/// Today's date key (`YYYY-MM-DD`) in the user's local timezone.
pub fn today_key() -> String {
  Local::now().date_naive().format("%Y-%m-%d").to_string()
}

/// Latest cumulative seconds per day (last write wins), oldest first.
fn load_days() -> BTreeMap<String, u64> {
  let mut days: BTreeMap<String, u64> = BTreeMap::new();
  let Ok(path) = days_file() else {
    return days;
  };
  let Ok(file) = OpenOptions::new().read(true).open(path) else {
    return days;
  };
  for line in io::BufReader::new(file).lines() {
    let Ok(line) = line else { continue };
    let Ok(DayEvent::ReadingDay { day, seconds }) =
      serde_json::from_str::<DayEvent>(&line)
    else {
      continue;
    };
    days.insert(day, seconds);
  }
  days
}

/// Add `delta` seconds to today's bucket and return the new cumulative total
/// for today. Append-only (last line for a day wins on load).
pub fn add_today_seconds(delta: u64) -> u64 {
  let day = today_key();
  let current = load_days().get(&day).copied().unwrap_or(0);
  let total = current.saturating_add(delta);
  if let Ok(path) = days_file()
    && let Ok(mut file) =
      OpenOptions::new().create(true).append(true).open(path)
  {
    let event = DayEvent::ReadingDay { day, seconds: total };
    if let Ok(serialized) = serde_json::to_string(&event) {
      let _ = file.write_all(serialized.as_bytes());
      let _ = file.write_all(b"\n");
    }
  }
  total
}

/// Compact human duration like `3h 12m`, `12m`, or `45s`.
pub fn humanize_duration(seconds: u64) -> String {
  let hours = seconds / 3600;
  let minutes = (seconds % 3600) / 60;
  if hours > 0 {
    format!("{hours}h {minutes}m")
  } else if minutes > 0 {
    format!("{minutes}m")
  } else {
    format!("{seconds}s")
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn humanize_duration_formats_units() {
    assert_eq!(humanize_duration(45), "45s");
    assert_eq!(humanize_duration(12 * 60), "12m");
    assert_eq!(humanize_duration(3 * 3600 + 12 * 60), "3h 12m");
  }
}
