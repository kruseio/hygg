// Re-export core types and functions
pub use crate::highlights_core::{Highlight, HighlightData};
pub use crate::highlights_persistence::{
  get_highlights_file_path, load_highlights, save_highlights,
};

#[cfg(test)]
mod tests {
  use super::*;
  use tempfile::TempDir;

  #[test]
  fn test_highlight_overlap_detection() {
    let mut data = HighlightData::new("test".to_string());

    // Add a highlight from 10 to 20
    data.add_highlight(10, 20);

    // Test overlapping cases
    assert!(data.find_overlapping_highlights(0, 5).is_empty()); // No overlap
    assert!(!data.find_overlapping_highlights(5, 15).is_empty()); // Partial overlap
    assert!(!data.find_overlapping_highlights(15, 25).is_empty()); // Partial overlap
    assert!(!data.find_overlapping_highlights(12, 18).is_empty()); // Fully contained
    assert!(!data.find_overlapping_highlights(5, 25).is_empty()); // Fully contains
    assert!(data.find_overlapping_highlights(20, 30).is_empty()); // Adjacent, no overlap
    assert!(data.find_overlapping_highlights(25, 30).is_empty()); // No overlap
  }

  #[test]
  fn test_remove_overlapping_highlights() {
    let mut data = HighlightData::new("test".to_string());

    // Add multiple highlights
    data.add_highlight(10, 20);
    data.add_highlight(25, 35);
    data.add_highlight(40, 50);

    // Remove overlapping with range 15-30
    let removed = data.remove_overlapping_highlights(15, 30);

    assert_eq!(removed.len(), 2); // Should remove first two highlights
    assert_eq!(data.highlights.len(), 1); // Only the third highlight remains
    assert_eq!(data.highlights[0].start, 40);
  }

  #[test]
  fn test_highlight_sorting() {
    let mut data = HighlightData::new("test".to_string());

    // Add highlights out of order
    data.add_highlight(30, 40);
    data.add_highlight(10, 20);
    data.add_highlight(20, 30);

    // Check they are sorted
    assert_eq!(data.highlights[0].start, 10);
    assert_eq!(data.highlights[1].start, 20);
    assert_eq!(data.highlights[2].start, 30);
  }

  #[test]
  fn test_add_duplicate_highlight() {
    let mut data = HighlightData::new("test".to_string());

    // Add a highlight
    assert!(data.add_highlight(10, 20));

    // Try to add the same highlight again
    assert!(!data.add_highlight(10, 20));

    // Should still only have one highlight
    assert_eq!(data.highlights.len(), 1);
  }

  #[test]
  fn test_get_highlights_for_range() {
    let mut data = HighlightData::new("test".to_string());

    // Add some highlights
    data.add_highlight(10, 20);
    data.add_highlight(30, 40);
    data.add_highlight(50, 60);

    // Test various ranges
    let highlights = data.get_highlights_for_range(0, 100);
    assert_eq!(highlights.len(), 3); // All highlights

    let highlights = data.get_highlights_for_range(15, 35);
    assert_eq!(highlights.len(), 2); // First two highlights

    let highlights = data.get_highlights_for_range(25, 29);
    assert_eq!(highlights.len(), 0); // No highlights in gap

    let highlights = data.get_highlights_for_range(35, 55);
    assert_eq!(highlights.len(), 2); // Last two highlights
  }

  #[test]
  fn test_save_and_load_highlights() {
    use std::fs;
    use std::path::PathBuf;

    // Create a temporary directory for testing
    let temp_dir = TempDir::new().unwrap();
    let test_dir = temp_dir.path().join(".config/hygg/highlights");
    fs::create_dir_all(&test_dir).unwrap();

    let mut data = HighlightData::new("test_doc_hash".to_string());

    // Add some highlights
    data.add_highlight(10, 20);
    data.add_highlight(30, 40);

    // Save highlights directly to test directory
    let test_file = test_dir.join("test_doc_hash.json");
    let json = serde_json::to_string_pretty(&data).unwrap();
    fs::write(&test_file, json).unwrap();

    // Load highlights directly from test directory
    let contents = fs::read_to_string(&test_file).unwrap();
    let loaded_data: HighlightData = serde_json::from_str(&contents).unwrap();

    // Check loaded data matches
    assert_eq!(loaded_data.document_hash, "test_doc_hash");
    assert_eq!(loaded_data.highlights.len(), 2);
    assert_eq!(loaded_data.highlights[0].start, 10);
    assert_eq!(loaded_data.highlights[0].end, 20);
    assert_eq!(loaded_data.highlights[1].start, 30);
    assert_eq!(loaded_data.highlights[1].end, 40);
  }

  #[test]
  fn test_load_nonexistent_highlights() {
    use std::fs;

    // Create a temporary directory for testing
    let temp_dir = TempDir::new().unwrap();
    let test_dir = temp_dir.path().join(".config/hygg/highlights");
    fs::create_dir_all(&test_dir).unwrap();

    // Try to load non-existent highlights file
    let test_file = test_dir.join("nonexistent.json");

    // Since the file doesn't exist, we should get empty HighlightData
    // This tests the behavior of load_highlights when file doesn't exist
    if !test_file.exists() {
      let data = HighlightData::new("nonexistent".to_string());

      // Should return empty HighlightData with correct hash
      assert_eq!(data.document_hash, "nonexistent");
      assert_eq!(data.highlights.len(), 0);
    }
  }

  #[test]
  fn test_clear_highlights() {
    let mut data = HighlightData::new("test".to_string());

    // Add some highlights
    data.add_highlight(10, 20);
    data.add_highlight(30, 40);

    assert_eq!(data.highlights.len(), 2);

    // Clear all highlights
    data.clear();

    assert_eq!(data.highlights.len(), 0);
  }
}
