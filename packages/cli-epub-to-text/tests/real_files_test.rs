use cli_epub_to_text::{EpubError, epub_to_text};
use std::path::Path;

#[test]
fn test_standard_epub_file() {
  // Test with our standard EPUB test file
  let test_file = Path::new(env!("CARGO_MANIFEST_DIR"))
    .parent()
    .unwrap()
    .parent()
    .unwrap()
    .join("test-data/epub/test-standard.epub");

  if !test_file.exists() {
    eprintln!(
      "Skipping test: test-standard.epub not found. Run create_test_epub test first."
    );
    return;
  }

  let result = epub_to_text(test_file.to_str().unwrap());
  assert!(result.is_ok(), "Failed to read standard EPUB: {:?}", result.err());

  let content = result.unwrap();

  // Verify content extraction
  assert!(content.contains("Hygg Test EPUB"), "Should contain book title");
  assert!(
    content.contains("Chapter 1: Introduction"),
    "Should contain chapter 1"
  );
  assert!(
    content.contains("Chapter 2: Formatting Tests"),
    "Should contain chapter 2"
  );
  assert!(
    content.contains("Chapter 3: Unicode and Special Characters"),
    "Should contain chapter 3"
  );

  // Verify Unicode content
  assert!(
    content.contains("Быстрая коричневая лиса"),
    "Should contain Russian text"
  );
  assert!(
    content.contains("素早い茶色のキツネ"),
    "Should contain Japanese text"
  );
  assert!(content.contains("😀"), "Should contain emoji");

  // Verify formatting conversion
  assert!(content.contains("bold text"), "Should extract bold text content");
  assert!(
    content.contains("italic text"),
    "Should extract italic text content"
  );

  // Verify lists are extracted
  assert!(content.contains("First item"), "Should extract list items");
  assert!(content.contains("Nested item"), "Should extract nested list items");

  // Verify table content
  assert!(content.contains("Text extraction"), "Should extract table content");
  assert!(content.contains("Supported"), "Should extract table cells");
}

#[test]
fn test_empty_epub() {
  // This should fail as the file doesn't exist
  let result = epub_to_text("nonexistent.epub");
  assert!(result.is_err());

  match result.unwrap_err() {
    EpubError::FileNotFound(_) => {}
    _ => panic!("Expected FileNotFound error"),
  }
}

#[test]
fn test_all_epub_files_in_test_data() {
  let test_data_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
    .parent()
    .unwrap()
    .parent()
    .unwrap()
    .join("test-data/epub");

  if !test_data_dir.exists() {
    eprintln!("test-data/epub directory not found, skipping test");
    return;
  }

  let mut tested_files = 0;

  if let Ok(entries) = std::fs::read_dir(&test_data_dir) {
    for entry in entries.flatten() {
      let path = entry.path();
      if path.extension().and_then(|s| s.to_str()) == Some("epub") {
        println!("Testing EPUB file: {:?}", path.file_name().unwrap());

        let result = epub_to_text(path.to_str().unwrap());
        assert!(
          result.is_ok(),
          "Failed to process EPUB {:?}: {:?}",
          path.file_name().unwrap(),
          result.err()
        );

        let content = result.unwrap();
        assert!(!content.trim().is_empty(), "EPUB should contain some text");

        tested_files += 1;
      }
    }
  }

  assert!(tested_files > 0, "No EPUB files found to test");
  println!("Successfully tested {} EPUB files", tested_files);
}
