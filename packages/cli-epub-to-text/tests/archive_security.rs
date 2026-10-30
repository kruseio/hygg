use cli_epub_to_text::epub_bytes_to_text;
use std::io::{Cursor, Write};
use zip::write::SimpleFileOptions;

fn book(href: &str, symlink: bool) -> Vec<u8> {
  let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
  let options = SimpleFileOptions::default()
    .compression_method(zip::CompressionMethod::Stored);
  let package = format!(
    r#"
    <opf:package xmlns:opf="http://www.idpf.org/2007/opf" version="3.0">
      <opf:manifest>
        <opf:item id="one" href="{href}" media-type="application/xhtml+xml"/>
        <opf:item id="two" href="second.xhtml" media-type="application/xhtml+xml"/>
      </opf:manifest>
      <opf:spine><opf:itemref idref="two"/><opf:itemref idref="missing"/>
        <opf:itemref idref="one"/></opf:spine>
    </opf:package>"#
  );
  for (name, content) in [
    (
      "META-INF/container.xml",
      r#"<container><rootfiles>
      <rootfile full-path="OPS/content.opf" media-type="application/oebps-package+xml"/>
      </rootfiles></container>"#,
    ),
    ("OPS/content.opf", &package),
    ("OPS/second.xhtml", "<p>Second chapter</p>"),
  ] {
    zip.start_file(name, options).unwrap();
    zip.write_all(content.as_bytes()).unwrap();
  }
  if symlink {
    zip
      .add_symlink("Text/first chapter.xhtml", "/etc/passwd", options)
      .unwrap();
  } else {
    zip.start_file("Text/first chapter.xhtml", options).unwrap();
    zip.write_all(b"<p>First chapter</p>").unwrap();
  }
  zip.finish().unwrap().into_inner()
}

#[test]
fn preserves_spine_order_namespaces_and_relative_encoded_paths() {
  let bytes = book("../Text/./first%20chapter.xhtml#start", false);
  let text = epub_bytes_to_text(&bytes).unwrap();
  assert_eq!(text, "Second chapter\n\nFirst chapter");
}

#[test]
fn rejects_cloaked_chapter_instead_of_silently_skipping_it() {
  let mut bytes = book("../Text/first%20chapter.xhtml", false);
  let name = b"Text/first chapter.xhtml";
  // Change only the local filename, leaving the central-directory name intact.
  let offset = bytes.windows(name.len()).position(|w| w == name).unwrap();
  bytes[offset] = b'X';
  let error = epub_bytes_to_text(&bytes).unwrap_err().to_string();
  assert!(error.contains("header"), "{error}");
}

#[test]
fn rejects_cloaked_container() {
  let mut bytes = book("../Text/first%20chapter.xhtml", false);
  bytes[30] = b'X'; // First local header's filename, not its central entry.
  assert!(epub_bytes_to_text(&bytes).is_err());
}

#[test]
fn rejects_symlink_resources_and_paths_outside_the_archive() {
  assert!(
    epub_bytes_to_text(&book("../Text/first%20chapter.xhtml", true)).is_err()
  );
  for href in ["../../outside.xhtml", "/etc/passwd", "file:///etc/passwd"] {
    assert!(epub_bytes_to_text(&book(href, false)).is_err(), "{href}");
  }
}
