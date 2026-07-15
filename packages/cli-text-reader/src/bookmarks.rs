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
