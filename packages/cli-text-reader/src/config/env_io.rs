//! Env-file read/rewrite helpers for `~/.config/hygg/.env`. Split out from the
//! config module to keep each file within the repository's per-file line
//! budget; behaviour is unchanged.

use std::fs;

/// Rewrite `~/.config/hygg/.env`, setting the given keys and **preserving every
/// other key** already in the file. This is the fix for the previous whole-file
/// overwrite that silently dropped unmanaged keys (e.g. `TTS_VOICE`, and now
/// the server keys when the other writer runs).
pub(super) fn write_env_preserving(
  managed: &[(&str, String)],
) -> Result<(), Box<dyn std::error::Error>> {
  let path = super::get_config_env_path()?;
  let existing = fs::read_to_string(&path).unwrap_or_default();
  fs::write(path, merge_env_content(&existing, managed))?;
  Ok(())
}

/// Pure merge: emit `managed` keys, then every `KEY=VALUE` line from `existing`
/// whose key is not managed (comments and blanks are dropped).
fn merge_env_content(existing: &str, managed: &[(&str, String)]) -> String {
  let managed_keys: std::collections::HashSet<&str> =
    managed.iter().map(|(key, _)| *key).collect();
  let mut out = String::new();
  for (key, value) in managed {
    out.push_str(&format!("{key}={value}\n"));
  }
  for line in existing.lines() {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
      continue;
    }
    if let Some((key, _)) = trimmed.split_once('=')
      && !managed_keys.contains(key.trim())
    {
      out.push_str(trimmed);
      out.push('\n');
    }
  }
  out
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn merge_updates_managed_and_preserves_unmanaged() {
    let existing =
      "ENABLE_TUTORIAL=true\nTTS_VOICE=af_heart\nSERVER_URL=http://h\n";
    let managed = [("ENABLE_TUTORIAL", "false".to_string())];
    let out = merge_env_content(existing, &managed);
    assert!(out.contains("ENABLE_TUTORIAL=false"));
    assert!(!out.contains("ENABLE_TUTORIAL=true"));
    // Unmanaged keys (TTS + the other writer's server keys) survive.
    assert!(out.contains("TTS_VOICE=af_heart"));
    assert!(out.contains("SERVER_URL=http://h"));
  }

  #[test]
  fn merge_drops_comments_and_blanks_but_keeps_other_keys() {
    let existing = "# a comment\n\nPDF_OCR=true\nDEVICE_ID=abc\n";
    let managed = [("PDF_OCR", "false".to_string())];
    let out = merge_env_content(existing, &managed);
    assert!(!out.contains("# a comment"));
    assert!(out.contains("PDF_OCR=false"));
    assert!(out.contains("DEVICE_ID=abc"));
  }
}
