use cli_epub_to_text::epub_to_text;
use std::fs;
use std::io::Write;
use tempfile::TempDir;

#[test]
fn debug_epub_creation() {
  let temp_dir = TempDir::new().unwrap();
  let epub_path = temp_dir.path().join("debug.epub");

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
    <rootfile full-path="content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#).unwrap();

  // Add content.opf
  zip.start_file("content.opf", options).unwrap();
  zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" unique-identifier="BookId" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
    <dc:title>Debug Book</dc:title>
    <dc:identifier id="BookId" opf:scheme="UUID">debug-book-uuid</dc:identifier>
    <dc:language>en</dc:language>
  </metadata>
  <manifest>
    <item id="chapter1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="chapter1"/>
  </spine>
</package>"#).unwrap();

  // Add chapter1.xhtml
  zip.start_file("chapter1.xhtml", options).unwrap();
  zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.1//EN" "http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd">
<html xmlns="http://www.w3.org/1999/xhtml">
<head>
  <title>Chapter 1</title>
</head>
<body>
  <h1>Chapter 1 Title</h1>
  <p>This is the content of chapter 1. It contains some text for testing.</p>
</body>
</html>"#).unwrap();

  zip.finish().unwrap();

  // Now test
  let result = epub_to_text(&epub_path.to_string_lossy());
  match result {
    Ok(text) => {
      println!("SUCCESS: Got text: '{text}'");
      assert!(!text.is_empty(), "Should have some content");
    }
    Err(e) => {
      println!("ERROR: {e:?}");
      panic!("Failed to convert EPUB: {e:?}");
    }
  }
}
