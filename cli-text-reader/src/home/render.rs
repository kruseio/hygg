use crate::library::{LibraryEntry, load_index};
use crate::progress::load_progress;
use crate::reading_stats::{FINISHED_PERCENTAGE, humanize_duration};

/// A library entry joined with its current reading progress, ready to display.
pub struct HomeItem {
  pub entry: LibraryEntry,
  pub percentage: f64,
  /// Cumulative active reading time for this document on this device, seconds.
  pub reading_seconds: u64,
}

/// Recently-read documents (newest first) joined with saved progress.
pub fn load_home_items() -> Vec<HomeItem> {
  load_index()
    .into_iter()
    .map(|entry| {
      let (percentage, reading_seconds) = load_progress(entry.document_hash)
        .map(|p| (p.percentage, p.reading_time_seconds))
        .unwrap_or((0.0, 0));
      HomeItem { entry, percentage, reading_seconds }
    })
    .collect()
}

fn truncate_chars(text: &str, max: usize) -> String {
  if text.chars().count() <= max {
    text.to_string()
  } else {
    text.chars().take(max.saturating_sub(1)).chain(['…']).collect()
  }
}

/// A compact unicode progress bar of `cells` width (e.g. `█████░░░`).
fn progress_bar(percentage: f64, cells: usize) -> String {
  let ratio = (percentage / 100.0).clamp(0.0, 1.0);
  let filled = ((ratio * cells as f64).round() as usize).min(cells);
  let mut bar = String::with_capacity(cells);
  bar.extend(std::iter::repeat_n('█', filled));
  bar.extend(std::iter::repeat_n('░', cells - filled));
  bar
}

/// `1.2 MB` / `834 KB` / `512 B`, matching the PWA home. `None` when the source
/// file can't be stat-ed (e.g. it has moved).
fn file_size_label(entry: &LibraryEntry) -> Option<String> {
  let path = entry.source_path.as_deref()?;
  let bytes = std::fs::metadata(path).ok()?.len() as f64;
  Some(if bytes >= 1_048_576.0 {
    format!("{:.1} MB", bytes / 1_048_576.0)
  } else if bytes >= 1024.0 {
    format!("{:.0} KB", bytes / 1024.0)
  } else {
    format!("{bytes} B")
  })
}

/// Relative "last read" label from an epoch-millis timestamp: `just now`,
/// `13m ago`, `3h ago`, `2d ago`. Empty when unset or in the future.
fn last_read_label(last_opened: i64) -> String {
  let now = chrono::Utc::now().timestamp_millis();
  if last_opened <= 0 || last_opened > now {
    return String::new();
  }
  let mins = ((now - last_opened) / 60_000).max(0);
  match mins {
    0 => "just now".to_string(),
    1..=59 => format!("{mins}m ago"),
    60..=1439 => format!("{}h ago", mins / 60),
    _ => format!("{}d ago", mins / 1440),
  }
}

/// The dashboard stat line mirroring the PWA home: total reading time, document
/// count, how many were started, and how many finished — derived from the
/// (already reconciled) items so it always agrees with the cards below.
pub fn stats_line(items: &[HomeItem]) -> String {
  let seconds: u64 = items.iter().map(|i| i.reading_seconds).sum();
  let started = items.iter().filter(|i| i.percentage > 0.0).count();
  let finished =
    items.iter().filter(|i| i.percentage >= FINISHED_PERCENTAGE).count();
  format!(
    "  {} reading · {} documents · {} started · {} finished",
    humanize_duration(seconds),
    items.len(),
    started,
    finished,
  )
}

/// The two display lines for one library card: the title, then a progress bar
/// with percentage, format, size, and when it was last read — a terminal
/// rendering of the PWA's book card. Both lines are truncated to `width`.
pub fn item_card(item: &HomeItem, width: usize) -> [String; 2] {
  let pct = item.percentage.round().clamp(0.0, 100.0) as i64;
  let bar = progress_bar(item.percentage, 10);
  let mut meta =
    format!("{bar} {pct}% · {}", item.entry.source_kind.to_uppercase());
  if let Some(size) = file_size_label(&item.entry) {
    meta.push_str(&format!(" · {size}"));
  }
  let last = last_read_label(item.entry.last_opened);
  if !last.is_empty() {
    meta.push_str(&format!(" · Last read {last}"));
  }
  [truncate_chars(&item.entry.title, width), truncate_chars(&meta, width)]
}

#[cfg(test)]
mod tests {
  use super::*;

  fn item(title: &str, pct: f64, kind: &str) -> HomeItem {
    let mut entry = LibraryEntry::from_path(1, None, "/x", 1);
    entry.title = title.to_string();
    entry.source_kind = kind.to_string();
    entry.source_path = None; // no file to stat in unit tests
    HomeItem { entry, percentage: pct, reading_seconds: 0 }
  }

  #[test]
  fn card_includes_title_percentage_and_format() {
    let [title, meta] = item_card(&item("Dune", 37.4, "epub"), 80);
    assert_eq!(title, "Dune");
    assert!(meta.contains("37%"));
    assert!(meta.contains("EPUB"));
  }

  #[test]
  fn card_truncates_each_line_to_width() {
    let [title, meta] =
      item_card(&item("A very long title indeed", 10.0, "pdf"), 16);
    assert!(title.chars().count() <= 16);
    assert!(meta.chars().count() <= 16);
  }

  #[test]
  fn progress_bar_fills_proportionally() {
    assert_eq!(progress_bar(0.0, 4), "░░░░");
    assert_eq!(progress_bar(100.0, 4), "████");
    assert_eq!(progress_bar(50.0, 4), "██░░");
  }

  #[test]
  fn last_read_label_buckets_by_age() {
    let now = chrono::Utc::now().timestamp_millis();
    assert_eq!(last_read_label(now - 5 * 60_000), "5m ago");
    assert_eq!(last_read_label(now - 3 * 3_600_000), "3h ago");
    assert_eq!(last_read_label(now - 2 * 86_400_000), "2d ago");
    assert_eq!(last_read_label(0), "");
  }

  #[test]
  fn stats_line_counts_started_and_finished() {
    let items = vec![
      item("a", 0.0, "pdf"),
      item("b", 23.0, "pdf"),
      item("c", 99.0, "pdf"),
    ];
    let line = stats_line(&items);
    assert!(line.contains("3 documents"));
    assert!(line.contains("2 started"), "line was: {line}");
    assert!(line.contains("1 finished"), "line was: {line}");
  }
}
