use crate::debug::{debug_log, debug_log_error, debug_log_event};
use crate::highlights_core::HighlightData;
use std::fs;
use std::path::PathBuf;

pub fn get_highlights_file_path(document_hash: &str) -> PathBuf {
  let mut config_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
  config_dir.push(".config");
  config_dir.push("hygg");
  config_dir.push("highlights");

  // Ensure the directory exists
  if let Err(e) = fs::create_dir_all(&config_dir) {
    debug_log_error(
      "highlights",
      &format!("Failed to create highlights directory: {e}"),
    );
  }

  config_dir.push(format!("{document_hash}.json"));
  config_dir
}

pub fn save_highlights(highlight_data: &HighlightData) -> Result<(), String> {
  let file_path = get_highlights_file_path(&highlight_data.document_hash);

  debug_log_event(
    "highlights",
    "save",
    &format!("Saving highlights to: {file_path:?}"),
  );

  let json = serde_json::to_string_pretty(highlight_data)
    .map_err(|e| format!("Failed to serialize highlights: {e}"))?;

  fs::write(&file_path, json).map_err(|e| {
    debug_log_error(
      "highlights",
      &format!("Failed to write highlights file: {e}"),
    );
    format!("Failed to save highlights: {e}")
  })?;

  debug_log(
    "highlights",
    &format!(
      "Successfully saved {} highlights",
      highlight_data.highlights.len()
    ),
  );
  Ok(())
}

pub fn load_highlights(document_hash: &str) -> Result<HighlightData, String> {
  let file_path = get_highlights_file_path(document_hash);

  debug_log_event(
    "highlights",
    "load",
    &format!("Loading highlights from: {file_path:?}"),
  );

  if !file_path.exists() {
    debug_log(
      "highlights",
      "Highlights file does not exist, creating new HighlightData",
    );
    return Ok(HighlightData::new(document_hash.to_string()));
  }

  let contents = fs::read_to_string(&file_path).map_err(|e| {
    debug_log_error(
      "highlights",
      &format!("Failed to read highlights file: {e}"),
    );
    format!("Failed to read highlights: {e}")
  })?;

  let mut highlight_data: HighlightData = serde_json::from_str(&contents)
    .map_err(|e| {
      debug_log_error(
        "highlights",
        &format!("Failed to parse highlights JSON: {e}"),
      );
      format!("Failed to parse highlights: {e}")
    })?;

  // Ensure highlights are sorted
  highlight_data.highlights.sort_by_key(|h| h.start);

  debug_log(
    "highlights",
    &format!(
      "Successfully loaded {} highlights",
      highlight_data.highlights.len()
    ),
  );
  Ok(highlight_data)
}
