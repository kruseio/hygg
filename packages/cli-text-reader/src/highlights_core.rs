use crate::debug::{
  debug_log, debug_log_error, debug_log_event, debug_log_state,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Highlight {
  pub start: usize,    // Absolute position in text
  pub end: usize,      // Absolute position in text
  pub created_at: u64, // Unix timestamp
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HighlightData {
  pub document_hash: String,
  pub highlights: Vec<Highlight>,
  pub version: u32, // For future compatibility
}

impl HighlightData {
  pub fn new(document_hash: String) -> Self {
    debug_log(
      "highlights",
      &format!("Creating new HighlightData for document: {document_hash}"),
    );
    Self { document_hash, highlights: Vec::new(), version: 1 }
  }

  pub fn add_highlight(&mut self, start: usize, end: usize) -> bool {
    let highlight =
      Highlight { start, end, created_at: Utc::now().timestamp() as u64 };

    debug_log_event(
      "highlights",
      "add_highlight",
      &format!("Adding highlight: start={start}, end={end}"),
    );

    // Check if this exact highlight already exists
    if self.highlights.iter().any(|h| h.start == start && h.end == end) {
      debug_log("highlights", "Highlight already exists, skipping");
      return false;
    }

    self.highlights.push(highlight);

    // Keep highlights sorted by start position for efficient lookup
    self.highlights.sort_by_key(|h| h.start);

    debug_log_state(
      "highlights",
      "highlights_count",
      &self.highlights.len().to_string(),
    );
    true
  }

  pub fn remove_overlapping_highlights(
    &mut self,
    start: usize,
    end: usize,
  ) -> Vec<Highlight> {
    debug_log_event(
      "highlights",
      "remove_overlapping",
      &format!("Removing overlapping highlights: start={start}, end={end}"),
    );

    let mut removed = Vec::new();

    self.highlights.retain(|h| {
      // A highlight overlaps if: !(h.end <= start || h.start >= end)
      let overlaps = !(h.end <= start || h.start >= end);
      if overlaps {
        debug_log(
          "highlights",
          &format!(
            "Removing overlapping highlight: start={}, end={}",
            h.start, h.end
          ),
        );
        removed.push(h.clone());
      }
      !overlaps
    });

    debug_log_state("highlights", "removed_count", &removed.len().to_string());
    debug_log_state(
      "highlights",
      "highlights_count",
      &self.highlights.len().to_string(),
    );
    removed
  }

  pub fn find_overlapping_highlights(
    &self,
    start: usize,
    end: usize,
  ) -> Vec<&Highlight> {
    debug_log_event(
      "highlights",
      "find_overlapping",
      &format!("Finding overlapping highlights: start={start}, end={end}"),
    );

    let overlapping: Vec<&Highlight> = self
      .highlights
      .iter()
      .filter(|h| !(h.end <= start || h.start >= end))
      .collect();

    debug_log_state(
      "highlights",
      "overlapping_count",
      &overlapping.len().to_string(),
    );
    overlapping
  }

  pub fn get_highlights_for_range(
    &self,
    start: usize,
    end: usize,
  ) -> Vec<&Highlight> {
    self.highlights.iter().filter(|h| h.start < end && h.end > start).collect()
  }

  #[allow(dead_code)]
  pub fn clear(&mut self) {
    debug_log("highlights", "Clearing all highlights");
    self.highlights.clear();
  }

  // Clear all highlights and save (for tutorial)
  pub fn clear_all_highlights(&mut self) {
    debug_log("highlights", "Clearing all highlights for tutorial");
    self.highlights.clear();
    // Save empty highlights
    if let Err(e) = crate::highlights_persistence::save_highlights(self) {
      debug_log_error(
        "highlights",
        &format!("Failed to save cleared highlights: {e}"),
      );
    }
  }
}
