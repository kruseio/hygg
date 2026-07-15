use cli_epub_to_text::{EpubError, epub_to_text};
use std::fs;
use std::io::Write;
use std::path::Path;
use tempfile::TempDir;

/// Helper function to create a minimal valid EPUB file for testing
fn create_test_epub(temp_dir: &TempDir, content: &str) -> String {
  let epub_path = temp_dir.path().join("test.epub");

  // Create a minimal EPUB structure
  let zip_file = std::fs::File::create(&epub_path).unwrap();
  let mut zip = zip::ZipWriter::new(zip_file);

  // Add mimetype file (must be first and uncompressed)
  let options = zip::write::SimpleFileOptions::default()
    .compression_method(zip::CompressionMethod::Stored);
  zip.start_file("mimetype", options).unwrap();
  zip.write_all(b"application/epub+zip").unwrap();

  // Add META-INF/container.xml
  let options = zip::write::SimpleFileOptions::default();
  zip.start_file("META-INF/container.xml", options).unwrap();
  zip.write_all(br#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#).unwrap();

  // Add OEBPS/content.opf
  zip.start_file("OEBPS/content.opf", options).unwrap();
  zip.write_all(r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" unique-identifier="BookId" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
    <dc:title>Test Book</dc:title>
    <dc:identifier id="BookId" opf:scheme="UUID">test-book-uuid</dc:identifier>
    <dc:language>en</dc:language>
  </metadata>
  <manifest>
    <item id="chapter1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="chapter1"/>
  </spine>
</package>"#.to_string().as_bytes()).unwrap();

  // Add OEBPS/chapter1.xhtml
  zip.start_file("OEBPS/chapter1.xhtml", options).unwrap();
  zip.write_all(format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.1//EN" "http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd">
<html xmlns="http://www.w3.org/1999/xhtml">
<head>
  <title>Chapter 1</title>
</head>
<body>
  <h1>Chapter 1</h1>
  <p>{content}</p>
</body>
</html>"#).as_bytes()).unwrap();

  zip.finish().unwrap();

  epub_path.to_string_lossy().to_string()
}

#[test]
fn test_epub_to_text_success() {
  let temp_dir = TempDir::new().unwrap();
  let test_content = "This is a test paragraph with some sample text.";
  let epub_path = create_test_epub(&temp_dir, test_content);

  let result = epub_to_text(&epub_path);
  assert!(result.is_ok(), "Failed to convert EPUB: {:?}", result.err());

  let text = result.unwrap();
  assert!(text.contains("Chapter 1"), "Output should contain chapter title");
  assert!(text.contains(test_content), "Output should contain test content");
}

#[test]
fn test_epub_to_text_with_multiple_chapters() {
  let temp_dir = TempDir::new().unwrap();
  let epub_path = temp_dir.path().join("multi_chapter.epub");

  // Create EPUB with multiple chapters
  let zip_file = std::fs::File::create(&epub_path).unwrap();
  let mut zip = zip::ZipWriter::new(zip_file);

  // Add mimetype
  let options = zip::write::SimpleFileOptions::default()
    .compression_method(zip::CompressionMethod::Stored);
  zip.start_file("mimetype", options).unwrap();
  zip.write_all(b"application/epub+zip").unwrap();

  // Add META-INF/container.xml
  let options = zip::write::SimpleFileOptions::default();
  zip.start_file("META-INF/container.xml", options).unwrap();
  zip.write_all(br#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#).unwrap();

  // Add OEBPS/content.opf with multiple chapters
  zip.start_file("OEBPS/content.opf", options).unwrap();
  zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" unique-identifier="BookId" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
    <dc:title>Multi Chapter Book</dc:title>
    <dc:identifier id="BookId" opf:scheme="UUID">multi-chapter-uuid</dc:identifier>
    <dc:language>en</dc:language>
  </metadata>
  <manifest>
    <item id="chapter1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
    <item id="chapter2" href="chapter2.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="chapter1"/>
    <itemref idref="chapter2"/>
  </spine>
</package>"#).unwrap();

  // Add chapter1
  zip.start_file("OEBPS/chapter1.xhtml", options).unwrap();
  zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.1//EN" "http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd">
<html xmlns="http://www.w3.org/1999/xhtml">
<head><title>Chapter 1</title></head>
<body><h1>First Chapter</h1><p>Content of first chapter.</p></body>
</html>"#).unwrap();

  // Add chapter2
  zip.start_file("OEBPS/chapter2.xhtml", options).unwrap();
  zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.1//EN" "http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd">
<html xmlns="http://www.w3.org/1999/xhtml">
<head><title>Chapter 2</title></head>
<body><h1>Second Chapter</h1><p>Content of second chapter.</p></body>
</html>"#).unwrap();

  zip.finish().unwrap();

  let result = epub_to_text(&epub_path.to_string_lossy());
  assert!(
    result.is_ok(),
    "Failed to convert multi-chapter EPUB: {:?}",
    result.err()
  );

  let text = result.unwrap();
  assert!(text.contains("First Chapter"), "Should contain first chapter");
  assert!(text.contains("Second Chapter"), "Should contain second chapter");
  assert!(
    text.contains("Content of first chapter"),
    "Should contain first chapter content"
  );
  assert!(
    text.contains("Content of second chapter"),
    "Should contain second chapter content"
  );
}

#[test]
fn test_epub_to_text_file_not_found() {
  let result = epub_to_text("nonexistent_file.epub");
  assert!(result.is_err(), "Should return error for nonexistent file");

  match result.unwrap_err() {
    EpubError::FileNotFound(message) => {
      assert!(
        message.contains("nonexistent_file.epub"),
        "Error should contain the file path"
      );
      assert!(
        message.contains("Failed to resolve path"),
        "Error should indicate path resolution failure"
      );
    }
    _ => panic!("Expected FileNotFound error"),
  }
}

#[test]
fn test_epub_to_text_invalid_file() {
  let temp_dir = TempDir::new().unwrap();
  let invalid_epub_path = temp_dir.path().join("invalid.epub");

  // Create a file that's not a valid EPUB
  std::fs::write(&invalid_epub_path, b"This is not an EPUB file").unwrap();

  let result = epub_to_text(&invalid_epub_path.to_string_lossy());
  assert!(result.is_err(), "Should return error for invalid EPUB file");

  match result.unwrap_err() {
    EpubError::InvalidEpub(_) => {
      // Expected error type
    }
    _ => panic!("Expected InvalidEpub error"),
  }
}

#[test]
fn test_epub_to_text_empty_content() {
  let temp_dir = TempDir::new().unwrap();
  let epub_path = temp_dir.path().join("empty.epub");

  // Create EPUB with empty content
  let zip_file = std::fs::File::create(&epub_path).unwrap();
  let mut zip = zip::ZipWriter::new(zip_file);

  // Add minimal structure but with empty chapter
  let options = zip::write::SimpleFileOptions::default()
    .compression_method(zip::CompressionMethod::Stored);
  zip.start_file("mimetype", options).unwrap();
  zip.write_all(b"application/epub+zip").unwrap();

  let options = zip::write::SimpleFileOptions::default();
  zip.start_file("META-INF/container.xml", options).unwrap();
  zip.write_all(br#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#).unwrap();

  zip.start_file("OEBPS/content.opf", options).unwrap();
  zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" unique-identifier="BookId" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
    <dc:title>Empty Book</dc:title>
    <dc:identifier id="BookId" opf:scheme="UUID">empty-book-uuid</dc:identifier>
    <dc:language>en</dc:language>
  </metadata>
  <manifest>
    <item id="chapter1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="chapter1"/>
  </spine>
</package>"#).unwrap();

  // Add empty chapter
  zip.start_file("OEBPS/chapter1.xhtml", options).unwrap();
  zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.1//EN" "http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd">
<html xmlns="http://www.w3.org/1999/xhtml">
<head><title>Empty Chapter</title></head>
<body></body>
</html>"#).unwrap();

  zip.finish().unwrap();

  let result = epub_to_text(&epub_path.to_string_lossy());
  assert!(
    result.is_err(),
    "Should return error for EPUB with no readable content"
  );

  match result.unwrap_err() {
    EpubError::InvalidEpub(msg) => {
      assert!(
        msg.contains("No readable content"),
        "Error should mention no readable content"
      );
    }
    _ => panic!("Expected InvalidEpub error for empty content"),
  }
}

#[test]
fn test_epub_to_text_html_formatting() {
  let temp_dir = TempDir::new().unwrap();
  let content_with_html = r#"This is <strong>bold</strong> text and this is <em>italic</em> text.
    <p>This is a paragraph with <a href="http://example.com">a link</a>.</p>
    <ul><li>Item 1</li><li>Item 2</li></ul>"#;

  let epub_path = create_test_epub(&temp_dir, content_with_html);

  let result = epub_to_text(&epub_path);
  assert!(
    result.is_ok(),
    "Failed to convert EPUB with HTML formatting: {:?}",
    result.err()
  );

  let text = result.unwrap();
  // html2text should convert HTML to plain text
  assert!(text.contains("bold"), "Should contain text from strong tags");
  assert!(text.contains("italic"), "Should contain text from em tags");
  assert!(text.contains("Item 1"), "Should contain list items");
  assert!(text.contains("Item 2"), "Should contain list items");
}
