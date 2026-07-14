use super::*;

pub fn format_millis(value: i64) -> String {
  DateTime::<Utc>::from_timestamp_millis(value)
    .map(|dt| dt.format("%Y-%m-%d %H:%M UTC").to_string())
    .unwrap_or_else(|| value.to_string())
}

pub(crate) fn short_session_id(session_id: &str) -> String {
  session_id.chars().take(12).collect()
}

pub fn percent(part: i64, total: i64) -> i64 {
  if total <= 0 {
    0
  } else {
    ((part as f64 / total as f64) * 100.0).round() as i64
  }
}

pub(crate) fn format_date_utc(value: i64) -> String {
  DateTime::<Utc>::from_timestamp_millis(value)
    .map(|dt| dt.format("%Y-%m-%d").to_string())
    .unwrap_or_else(|| value.to_string())
}

/// Rough lines-per-page used to present line offsets as a friendly page count.
pub(crate) const LINES_PER_PAGE: i64 = 30;

/// Progress at or above this percentage counts a document as finished.
pub(crate) const FINISHED_PERCENTAGE: f64 = 98.0;

/// Compact human duration like `3h 12m`, `12m`, or `45s`.
pub(crate) fn humanize_duration(seconds: i64) -> String {
  let seconds = seconds.max(0);
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

pub fn format_bytes(bytes: i64) -> String {
  const KIB: f64 = 1024.0;
  let value = bytes.max(0) as f64;
  if value >= KIB * KIB * KIB {
    format!("{:.1} GB", value / KIB / KIB / KIB)
  } else if value >= KIB * KIB {
    format!("{:.1} MB", value / KIB / KIB)
  } else if value >= KIB {
    format!("{:.1} KB", value / KIB)
  } else {
    format!("{} B", bytes.max(0))
  }
}

/// Approximate stored size of a book's metadata row: its variable-length text
/// fields plus a fixed allowance for ids, timestamps and index entries. Used to
/// show what deleting metadata reclaims, not as a byte-exact figure.
pub(crate) fn metadata_bytes(book: &repo::books::BookRow) -> i64 {
  const ROW_OVERHEAD: i64 = 160;
  let text = book.content_hash.len()
    + book.title.len()
    + book.author.len()
    + book.format.len();
  text as i64 + ROW_OVERHEAD
}

pub(crate) fn yes_no(value: i64) -> &'static str {
  if value == 0 { "No" } else { "Yes" }
}

pub fn esc(input: &str) -> String {
  input
    .replace('&', "&amp;")
    .replace('<', "&lt;")
    .replace('>', "&gt;")
    .replace('"', "&quot;")
    .replace('\'', "&#39;")
}
