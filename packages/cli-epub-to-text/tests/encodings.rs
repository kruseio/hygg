//! XHTML chapters in encodings other than UTF-8: UTF-16 (allowed by EPUB 2),
//! legacy charsets named in the XML declaration or a <meta> tag, BOMs that
//! contradict the declaration, and stray U+FEFF at chapter starts.

use cli_epub_to_text::epub_bytes_to_text;
use std::io::{Cursor, Write};
use zip::write::SimpleFileOptions;

fn epub(chapters: &[Vec<u8>]) -> Vec<u8> {
  let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
  let stored = SimpleFileOptions::default()
    .compression_method(zip::CompressionMethod::Stored);
  zip.start_file("mimetype", stored).unwrap();
  zip.write_all(b"application/epub+zip").unwrap();

  let options = SimpleFileOptions::default();
  zip.start_file("META-INF/container.xml", options).unwrap();
  zip
    .write_all(
      br#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#,
    )
    .unwrap();

  let ids: Vec<String> =
    (1..=chapters.len()).map(|i| format!("c{i}")).collect();
  let manifest: String = ids
    .iter()
    .map(|id| {
      format!(
        r#"<item id="{id}" href="{id}.xhtml" media-type="application/xhtml+xml"/>"#
      )
    })
    .collect();
  let spine: String =
    ids.iter().map(|id| format!(r#"<itemref idref="{id}"/>"#)).collect();
  zip.start_file("OEBPS/content.opf", options).unwrap();
  zip
    .write_all(
      format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" unique-identifier="id" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Encodings</dc:title><dc:identifier id="id">enc</dc:identifier>
    <dc:language>en</dc:language>
  </metadata>
  <manifest>{manifest}</manifest>
  <spine>{spine}</spine>
</package>"#
      )
      .as_bytes(),
    )
    .unwrap();

  for (id, chapter) in ids.iter().zip(chapters) {
    zip.start_file(format!("OEBPS/{id}.xhtml"), options).unwrap();
    zip.write_all(chapter).unwrap();
  }
  zip.finish().unwrap().into_inner()
}

fn xhtml(declaration: &str, head: &str, body: &str) -> String {
  format!(
    "{declaration}<html xmlns=\"http://www.w3.org/1999/xhtml\"><head>{head}\
     <title>t</title></head><body>{body}</body></html>"
  )
}

fn latin1(text: &str) -> Vec<u8> {
  text.chars().map(|c| u8::try_from(u32::from(c)).unwrap()).collect()
}

#[test]
fn chapters_in_any_encoding_are_decoded() {
  let iso_8859_1 = latin1(&xhtml(
    r#"<?xml version="1.0" encoding="ISO-8859-1"?>"#,
    "",
    "<p>Ça m'a coûté très cher, señor.</p>",
  ));

  let mut utf16 = vec![0xFF, 0xFE];
  utf16.extend(
    xhtml(
      r#"<?xml version="1.0" encoding="UTF-16"?>"#,
      "",
      "<p>Καλημέρα κόσμε</p>",
    )
    .encode_utf16()
    .flat_map(u16::to_le_bytes),
  );

  // A UTF-8 BOM contradicting the declaration: the BOM wins.
  let mut bom_wins = b"\xEF\xBB\xBF".to_vec();
  bom_wins.extend(
    xhtml(r#"<?xml version="1.0" encoding="ISO-8859-1"?>"#, "", "<p>Größe</p>")
      .as_bytes(),
  );

  let cp1251_doc = xhtml(
    "",
    r#"<meta http-equiv="Content-Type" content="text/html; charset=windows-1251"/>"#,
    "<p>Привет, мир!</p>",
  );
  let (cp1251, _, _) = encoding_rs::WINDOWS_1251.encode(&cp1251_doc);

  let text =
    epub_bytes_to_text(&epub(&[iso_8859_1, utf16, bom_wins, cp1251.into()]))
      .unwrap();
  for expected in [
    "Ça m'a coûté très cher, señor.",
    "Καλημέρα κόσμε",
    "Größe",
    "Привет, мир!",
  ] {
    assert!(text.contains(expected), "missing {expected:?} in {text:?}");
  }
  assert!(!text.contains('\u{FFFD}'), "{text:?}");
}

#[test]
fn a_leading_feff_is_stripped_per_chapter_but_kept_mid_text() {
  let chapter = |body: &str| {
    let mut bytes = b"\xEF\xBB\xBF".to_vec();
    bytes.extend(xhtml("", "", body).as_bytes());
    bytes
  };
  let text = epub_bytes_to_text(&epub(&[
    chapter("<p>&#xFEFF;First chapter.</p>"),
    chapter("<p>&#xFEFF;Second a&#xFEFF;b chapter.</p>"),
  ]))
  .unwrap();
  assert!(text.starts_with("First chapter."), "{text:?}");
  assert!(text.contains("\n\nSecond a\u{FEFF}b chapter."), "{text:?}");
}
