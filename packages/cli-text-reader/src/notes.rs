//! Per-document notes, stored locally and keyed by `document_hash` exactly like
//! bookmarks and highlights. Works fully offline; later phases additionally
//! enqueue each note for background sync. The on-disk file is
//! `~/.config/hygg/notes/{document_hash}.json`.

use crate::utils::get_hygg_subdir_file;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Note {
  /// Stable id (uuid) so a note survives edits and cross-device sync.
  pub id: String,
  pub body: String,
  /// Document line the note was taken on, when known.
  #[serde(default)]
  pub line: Option<usize>,
  /// Unix epoch milliseconds.
  pub created_at: i64,
  pub updated_at: i64,
}

impl Note {
  pub fn new(body: String, line: Option<usize>) -> Self {
    let now = Utc::now().timestamp_millis();
    Self {
      id: Uuid::new_v4().to_string(),
      body,
      line,
      created_at: now,
      updated_at: now,
    }
  }
}

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, Eq)]
pub struct NoteData {
  pub notes: Vec<Note>,
}

fn get_notes_path(
  document_hash: u64,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
  get_hygg_subdir_file("notes", &format!("{document_hash}.json"))
}

pub fn load_notes(
  document_hash: u64,
) -> Result<NoteData, Box<dyn std::error::Error>> {
  let notes_path = get_notes_path(document_hash)?;
  if notes_path.exists() {
    let content = fs::read_to_string(notes_path)?;
    Ok(serde_json::from_str(&content)?)
  } else {
    Ok(NoteData::default())
  }
}

pub fn save_notes(
  document_hash: u64,
  notes: &NoteData,
) -> Result<(), Box<dyn std::error::Error>> {
  let notes_path = get_notes_path(document_hash)?;
  let content = serde_json::to_string_pretty(notes)?;
  fs::write(notes_path, content)?;
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn note_new_seeds_id_and_equal_timestamps() {
    let note = Note::new("first note".to_string(), Some(7));
    assert!(!note.id.is_empty());
    assert_eq!(note.body, "first note");
    assert_eq!(note.line, Some(7));
    assert_eq!(note.created_at, note.updated_at);
  }

  #[test]
  fn note_data_round_trips_through_json() {
    let data = NoteData {
      notes: vec![
        Note::new("alpha".to_string(), None),
        Note::new("beta".to_string(), Some(3)),
      ],
    };
    let json = serde_json::to_string(&data).unwrap();
    let restored: NoteData = serde_json::from_str(&json).unwrap();
    assert_eq!(data, restored);
  }

  #[test]
  fn legacy_json_without_line_field_deserializes() {
    let json =
      r#"{"notes":[{"id":"x","body":"hi","created_at":1,"updated_at":1}]}"#;
    let data: NoteData = serde_json::from_str(json).unwrap();
    assert_eq!(data.notes.len(), 1);
    assert_eq!(data.notes[0].line, None);
  }
}
