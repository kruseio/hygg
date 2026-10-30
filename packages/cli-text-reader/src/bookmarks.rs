use crate::utils::get_hygg_subdir_file;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Default)]
pub struct BookmarkData {
  pub marks: HashMap<char, (usize, usize)>, // mark -> (line, col)
}

fn get_bookmarks_path(
  document_hash: u64,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
  get_hygg_subdir_file("bookmarks", &format!("{document_hash}.json"))
}

pub fn load_bookmarks(
  document_hash: u64,
) -> Result<BookmarkData, Box<dyn std::error::Error>> {
  let bookmarks_path = get_bookmarks_path(document_hash)?;

  if bookmarks_path.exists() {
    let content = fs::read_to_string(bookmarks_path)?;
    let bookmarks: BookmarkData = serde_json::from_str(&content)?;
    Ok(bookmarks)
  } else {
    Ok(BookmarkData::default())
  }
}

pub fn save_bookmarks(
  document_hash: u64,
  bookmarks: &HashMap<char, (usize, usize)>,
) -> Result<(), Box<dyn std::error::Error>> {
  let bookmarks_path = get_bookmarks_path(document_hash)?;

  let bookmark_data = BookmarkData { marks: bookmarks.clone() };

  let content = serde_json::to_string_pretty(&bookmark_data)?;
  fs::write(bookmarks_path, content)?;
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::BookmarkData;
  use crate::editor::Editor;
  use crate::editor::char_boundary::Pos;

  // Bookmarks files written before typed columns: a mark is `[line, col]`,
  // with `col` a raw byte offset.
  const ON_DISK: &str = r#"{"marks":{"a":[5,3],"b":[0,9]}}"#;

  #[test]
  fn marks_json_format_is_unchanged() {
    let data: BookmarkData = serde_json::from_str(ON_DISK).unwrap();
    assert_eq!(data.marks.get(&'a'), Some(&(5, 3)));
    assert_eq!(data.marks.get(&'b'), Some(&(0, 9)));

    // Written back in the same shape, so older builds and other devices
    // still read it.
    let one = BookmarkData { marks: [('a', (5, 3))].into_iter().collect() };
    assert_eq!(
      serde_json::to_string(&one).unwrap(),
      r#"{"marks":{"a":[5,3]}}"#
    );
    let back: serde_json::Value =
      serde_json::from_str(&serde_json::to_string(&data).unwrap()).unwrap();
    let want: serde_json::Value = serde_json::from_str(ON_DISK).unwrap();
    assert_eq!(back, want);
  }

  #[test]
  fn a_stored_mid_character_column_is_settled_on_jump() {
    // Mark `b` was stored at byte 9, inside the 'ó' of "canción" (bytes
    // 8..10), as a mark synced from a device with a different layout can be.
    let data: BookmarkData = serde_json::from_str(ON_DISK).unwrap();
    let mut editor = Editor::new(vec!["La canción está".to_string()], 80);
    editor.offset = 0;
    editor.cursor_y = 0;
    let raw = data.marks[&'b'];
    editor.move_to_position(Pos::from_raw(&editor.lines, raw));
    assert_eq!(editor.cursor_x.raw(), 8);
    // And a mark on a line that does not exist here does not panic.
    let target = Pos::from_raw(&editor.lines, data.marks[&'a']);
    assert_eq!(target.to_raw(), (5, 0));
  }
}
