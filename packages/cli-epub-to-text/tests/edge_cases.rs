use cli_epub_to_text::{EpubError, epub_to_text};
use std::io::Write;
use tempfile::TempDir;

/// Test EPUB with malformed XHTML that html2text should handle gracefully
#[test]
fn test_malformed_xhtml() {
  let temp_dir = TempDir::new().unwrap();
  let epub_path = temp_dir.path().join("malformed.epub");

  let zip_file = std::fs::File::create(&epub_path).unwrap();
  let mut zip = zip::ZipWriter::new(zip_file);

  // Add mimetype
  let options = zip::write::SimpleFileOptions::default()
    .compression_method(zip::CompressionMethod::Stored);
  zip.start_file("mimetype", options).unwrap();
  zip.write_all(b"application/epub+zip").unwrap();

  // Add container.xml
  let options = zip::write::SimpleFileOptions::default();
  zip.start_file("META-INF/container.xml", options).unwrap();
  zip.write_all(br#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#).unwrap();

  // Add content.opf
  zip.start_file("OEBPS/content.opf", options).unwrap();
  zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" unique-identifier="BookId" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Malformed Book</dc:title>
    <dc:identifier id="BookId">malformed-book</dc:identifier>
    <dc:language>en</dc:language>
  </metadata>
  <manifest>
    <item id="chapter1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="chapter1"/>
  </spine>
</package>"#).unwrap();

  // Add malformed XHTML (unclosed tags, mixed content)
  zip.start_file("OEBPS/chapter1.xhtml", options).unwrap();
  zip
    .write_all(
      br#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml">
<head><title>Test</title></head>
<body>
  <h1>Chapter Title</h1>
  <p>This is a paragraph with <strong>bold text that's not closed
  <p>Another paragraph with text.
  <div>Some content in a div</div>
  Text outside any tags.
</body>
</html>"#,
    )
    .unwrap();

  zip.finish().unwrap();

  let result = epub_to_text(&epub_path.to_string_lossy());
  // html2text should handle malformed HTML gracefully
  assert!(
    result.is_ok(),
    "Should handle malformed XHTML gracefully: {:?}",
    result.err()
  );

  let text = result.unwrap();
  assert!(text.contains("Chapter Title"), "Should extract text from heading");
  assert!(
    text.contains("This is a paragraph"),
    "Should extract paragraph text"
  );
  assert!(
    text.contains("Another paragraph"),
    "Should extract second paragraph"
  );
}

/// Test EPUB with Unicode characters and special symbols
#[test]
fn test_unicode_content() {
  let temp_dir = TempDir::new().unwrap();
  let epub_path = temp_dir.path().join("unicode.epub");

  let zip_file = std::fs::File::create(&epub_path).unwrap();
  let mut zip = zip::ZipWriter::new(zip_file);

  // Add mimetype
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
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Unicode Test Book</dc:title>
    <dc:identifier id="BookId">unicode-test</dc:identifier>
    <dc:language>en</dc:language>
  </metadata>
  <manifest>
    <item id="chapter1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="chapter1"/>
  </spine>
</package>"#).unwrap();

  zip.start_file("OEBPS/chapter1.xhtml", options).unwrap();
  let unicode_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml">
<head><title>Unicode Test</title></head>
<body>
  <h1>Тест с русскими символами</h1>
  <p>This contains émojis: 📚📖🔤 and special characters: àáâãäåæçèéêë</p>
  <p>Math symbols: ∑∞∂∇∈∉⊆⊇∩∪ and arrows: ←→↑↓</p>
  <p>Currency: €£¥¢ and other symbols: ©®™§¶†‡</p>
</body>
</html>"#;
  zip.write_all(unicode_content.as_bytes()).unwrap();

  zip.finish().unwrap();

  let result = epub_to_text(&epub_path.to_string_lossy());
  assert!(result.is_ok(), "Should handle Unicode content: {:?}", result.err());

  let text = result.unwrap();
  assert!(
    text.contains("Тест с русскими символами"),
    "Should preserve Cyrillic text"
  );
  assert!(text.contains("📚📖🔤"), "Should preserve emojis");
  assert!(text.contains("àáâãäåæçèéêë"), "Should preserve accented characters");
  assert!(text.contains("∑∞∂∇"), "Should preserve math symbols");
  assert!(text.contains("€£¥¢"), "Should preserve currency symbols");
}

/// Test EPUB with deeply nested HTML structure
#[test]
fn test_nested_html_structure() {
  let temp_dir = TempDir::new().unwrap();
  let epub_path = temp_dir.path().join("nested.epub");

  let zip_file = std::fs::File::create(&epub_path).unwrap();
  let mut zip = zip::ZipWriter::new(zip_file);

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
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Nested Structure Book</dc:title>
    <dc:identifier id="BookId">nested-test</dc:identifier>
    <dc:language>en</dc:language>
  </metadata>
  <manifest>
    <item id="chapter1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="chapter1"/>
  </spine>
</package>"#).unwrap();

  zip.start_file("OEBPS/chapter1.xhtml", options).unwrap();
  let nested_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml">
<head><title>Nested Test</title></head>
<body>
  <div class="chapter">
    <header>
      <h1>Complex Chapter</h1>
      <p class="subtitle">With nested elements</p>
    </header>
    <section class="content">
      <article>
        <h2>Section 1</h2>
        <div class="paragraph-group">
          <p>First paragraph with <span class="highlight">highlighted <em>nested emphasis</em></span> text.</p>
          <blockquote>
            <p>A quote within a blockquote with <strong>bold text</strong>.</p>
            <footer>
              <cite>Citation source</cite>
            </footer>
          </blockquote>
        </div>
        <aside class="sidebar">
          <h3>Sidebar content</h3>
          <ul>
            <li>List item with nested link</li>
            <li>Another item</li>
          </ul>
        </aside>
      </article>
    </section>
  </div>
</body>
</html>"#;
  zip.write_all(nested_content.as_bytes()).unwrap();

  zip.finish().unwrap();

  let result = epub_to_text(&epub_path.to_string_lossy());
  assert!(
    result.is_ok(),
    "Should handle deeply nested HTML: {:?}",
    result.err()
  );

  let text = result.unwrap();
  assert!(text.contains("Complex Chapter"), "Should extract main heading");
  assert!(text.contains("With nested elements"), "Should extract subtitle");
  assert!(text.contains("First paragraph"), "Should extract paragraph text");
  assert!(text.contains("highlighted"), "Should extract span content");
  assert!(text.contains("nested emphasis"), "Should extract nested emphasis");
  assert!(text.contains("A quote within"), "Should extract blockquote content");
  assert!(text.contains("Citation source"), "Should extract cite content");
  assert!(text.contains("Sidebar content"), "Should extract aside content");
  assert!(text.contains("List item"), "Should extract list items");
}
