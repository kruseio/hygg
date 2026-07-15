use crate::utils::get_hygg_config_file;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::fs::OpenOptions;
use std::hash::{Hash, Hasher};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
pub struct Progress {
  pub document_hash: u64,
  /// Local save timestamp in Unix milliseconds. Used to decide whether a
  /// server progress row is actually newer than the local restored position.
  #[serde(default)]
  pub updated_at: i64,
  pub offset: usize, /* This stores the actual line number (not viewport
                      * offset) */
  pub total_lines: usize,
  pub percentage: f64,
  #[serde(default)]
  pub viewport_offset: Option<usize>,
  #[serde(default)]
  pub cursor_y: Option<usize>,
  /// 1-based PDF page that the viewport was on at save time. Only populated
  /// for streaming PDF sessions; falls back to None for older saves and
  /// non-PDF documents.
  #[serde(default)]
  pub page: Option<u32>,
  /// Cursor's row within the page's rendered output (0-based). Paired with
  /// `page` to restore the exact spot regardless of which other pages are
  /// loaded when we re-open.
  #[serde(default)]
  pub line_in_page: Option<usize>,
  /// Non-whitespace character offset of the viewport-center line (page-local
  /// for PDFs, global otherwise) — the exact cross-width resume anchor. None
  /// for older saves. See `crate::word_anchor` / `hygg_shared::anchor`.
  #[serde(default)]
  pub word_offset: Option<usize>,
  /// Cumulative active reading time for this document on this device, in
  /// seconds. Seeded on open so accrual continues across sessions.
  #[serde(default)]
  pub reading_time_seconds: u64,
}

#[derive(Serialize, Deserialize)]
enum Event {
  UpdateProgress {
    timestamp: DateTime<Utc>,
    document_hash: u64,
    offset: usize,
    total_lines: usize,
    percentage: f64,
    #[serde(default)]
    viewport_offset: Option<usize>,
    #[serde(default)]
    cursor_y: Option<usize>,
    #[serde(default)]
    page: Option<u32>,
    #[serde(default)]
    line_in_page: Option<usize>,
    #[serde(default)]
    word_offset: Option<usize>,
    #[serde(default)]
    reading_time_seconds: u64,
  },
}

pub fn generate_hash<T: Hash>(t: &T) -> u64 {
  let mut s = DefaultHasher::new();
  t.hash(&mut s);
  s.finish()
}

fn get_progress_file_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
  get_hygg_config_file(".progress.jsonl")
}

#[allow(dead_code)]
pub fn save_progress(
  document_hash: u64,
  offset: usize,
  total_lines: usize,
) -> Result<(), Box<dyn std::error::Error>> {
  save_progress_with_viewport(document_hash, offset, total_lines, None, None)
}

#[allow(dead_code)]
pub fn save_progress_with_viewport(
  document_hash: u64,
  offset: usize, // This is the actual line number
  total_lines: usize,
  viewport_offset: Option<usize>,
  cursor_y: Option<usize>,
) -> Result<(), Box<dyn std::error::Error>> {
  // This convenience wrapper has no loaded document to measure characters
  // against, so it stores the coarse line-fraction. Used only by tests/tools —
  // the reader itself saves via `save_progress_snapshot` with the exact
  // character percent.
  let percentage = if total_lines > 0 {
    (offset as f64 / total_lines as f64) * 100.0
  } else {
    0.0
  };
  save_progress_full(
    document_hash,
    Utc::now().timestamp_millis(),
    offset,
    total_lines,
    percentage,
    viewport_offset,
    cursor_y,
    None,
    None,
    None,
    0,
  )
}

#[allow(clippy::too_many_arguments)]
pub fn save_progress_full(
  document_hash: u64,
  // Save timestamp in the server's clock domain (skew-corrected), so the
  // baseline this seeds on the next open compares correctly against a server
  // row. The reader passes its corrected `now`; the library reconcile passes
  // the adopted remote row's own timestamp.
  updated_at: i64,
  offset: usize,
  total_lines: usize,
  // Width-independent reading percent (non-whitespace-character fraction),
  // computed by the caller from the document so it matches the value synced to
  // the server and shown by peers. A caller without the loaded document (e.g.
  // the library reconcile) passes the value it already holds.
  percentage: f64,
  viewport_offset: Option<usize>,
  cursor_y: Option<usize>,
  page: Option<u32>,
  line_in_page: Option<usize>,
  word_offset: Option<usize>,
  reading_time_seconds: u64,
) -> Result<(), Box<dyn std::error::Error>> {
  let event = Event::UpdateProgress {
    timestamp: DateTime::from_timestamp_millis(updated_at)
      .unwrap_or_else(Utc::now),
    document_hash,
    offset,
    total_lines,
    percentage,
    viewport_offset,
    cursor_y,
    page,
    line_in_page,
    word_offset,
    reading_time_seconds,
  };
  let serialized = serde_json::to_string(&event)?;
  let progress_file_path = get_progress_file_path()?;
  let mut file =
    OpenOptions::new().create(true).append(true).open(progress_file_path)?;
  file.write_all(serialized.as_bytes())?;
  file.write_all(b"\n")?;
  Ok(())
}

pub fn load_progress(
  document_hash: u64,
) -> Result<Progress, Box<dyn std::error::Error>> {
  let progress_file_path = get_progress_file_path()?;
  let file = OpenOptions::new().read(true).open(progress_file_path)?;
  let reader = io::BufReader::new(file);
  let mut latest_progress: Option<Progress> = None;

  for line in reader.lines() {
    let line = line?;
    let Ok(event) = serde_json::from_str::<Event>(&line) else {
      continue;
    };

    let Event::UpdateProgress {
      timestamp,
      document_hash: hash,
      offset,
      total_lines,
      percentage,
      viewport_offset,
      cursor_y,
      page,
      line_in_page,
      word_offset,
      reading_time_seconds,
      ..
    } = event;

    if hash == document_hash {
      latest_progress = Some(Progress {
        document_hash: hash,
        updated_at: timestamp.timestamp_millis(),
        offset,
        total_lines,
        percentage,
        viewport_offset,
        cursor_y,
        page,
        line_in_page,
        word_offset,
        reading_time_seconds,
      });
    }
  }

  latest_progress
    .ok_or_else(|| "No progress found for the given document hash".into())
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::fs;
  use tempfile::tempdir;

  #[test]
  fn test_save_and_load_progress() {
    // Create a temporary directory for test
    let temp_dir = tempdir().unwrap();
    let temp_path = temp_dir.path().join(".progress.jsonl");

    // Mock the get_progress_file_path function by creating a test file
    let test_hash = 12345u64;
    let test_offset = 50usize;
    let test_total_lines = 100usize;

    // Save progress
    let percentage = (test_offset as f64 / test_total_lines as f64) * 100.0;
    let event = Event::UpdateProgress {
      timestamp: Utc::now(),
      document_hash: test_hash,
      offset: test_offset,
      total_lines: test_total_lines,
      percentage,
      viewport_offset: None,
      cursor_y: None,
      page: None,
      line_in_page: None,
      word_offset: None,
      reading_time_seconds: 0,
    };

    let serialized = serde_json::to_string(&event).unwrap();
    fs::write(&temp_path, format!("{serialized}\n")).unwrap();

    // Load progress by reading the file directly
    let file = OpenOptions::new().read(true).open(&temp_path).unwrap();
    let reader = io::BufReader::new(file);
    let mut loaded_progress: Option<Progress> = None;

    for line in reader.lines() {
      let line = line.unwrap();
      let event: Event = serde_json::from_str(&line).unwrap();

      let Event::UpdateProgress {
        timestamp,
        document_hash: hash,
        offset,
        total_lines,
        percentage,
        viewport_offset,
        cursor_y,
        page,
        line_in_page,
        word_offset,
        reading_time_seconds,
        ..
      } = event;

      if hash == test_hash {
        loaded_progress = Some(Progress {
          document_hash: hash,
          updated_at: timestamp.timestamp_millis(),
          offset,
          total_lines,
          percentage,
          viewport_offset,
          cursor_y,
          page,
          line_in_page,
          word_offset,
          reading_time_seconds,
        });
      }
    }

    // Verify the loaded progress
    let progress = loaded_progress.expect("Progress should be loaded");
    assert_eq!(progress.document_hash, test_hash);
    assert_eq!(progress.offset, test_offset);
    assert_eq!(progress.total_lines, test_total_lines);
    assert_eq!(progress.percentage, 50.0);
    assert!(progress.updated_at > 0);
  }
}
