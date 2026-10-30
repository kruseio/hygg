//! Decoding of HTML / XHTML / XML documents, whose bytes may name their own
//! encoding in an XML declaration (`<?xml version="1.0" encoding="..."?>`) or a
//! `<meta charset>` / `<meta http-equiv="Content-Type">` tag.

use super::{DecodedText, decode_with_declared};
use encoding_rs::{Encoding, REPLACEMENT, UTF_8, UTF_16BE, UTF_16LE};

/// How far into the document the declared charset is looked for, matching the
/// HTML prescan window.
const PRESCAN_LEN: usize = 1024;

/// Decode a markup document. Same as [`super::decode_text`], except that when
/// the bytes are not Unicode (no BOM, not UTF-16/32, not valid UTF-8) the
/// encoding the document declares is used before falling back to detection.
///
/// Valid UTF-8 deliberately wins over a declared legacy charset: files that
/// were converted to UTF-8 while keeping their old `encoding="ISO-8859-1"`
/// declaration are common in older EPUBs, and genuine Latin-1 text is almost
/// never valid UTF-8.
pub fn decode_markup(bytes: &[u8]) -> DecodedText {
  decode_with_declared(bytes, declared_encoding(bytes))
}

/// The encoding named in the first [`PRESCAN_LEN`] bytes, if it is one that can
/// apply to ASCII-compatible bytes. A UTF-16 label on bytes that reached this
/// point is wrong by construction (they are not UTF-16), and a UTF-8 label on
/// bytes that are not valid UTF-8 is equally wrong, so both are ignored in
/// favour of detection, as is the `replacement` pseudo-encoding.
fn declared_encoding(bytes: &[u8]) -> Option<&'static Encoding> {
  let head = &bytes[..bytes.len().min(PRESCAN_LEN)];
  let lower = head.to_ascii_lowercase();
  let label = find_label(&lower, b"encoding=")
    .or_else(|| find_label(&lower, b"charset="))?;
  let encoding = Encoding::for_label(label)?;
  let unusable = [UTF_8, UTF_16LE, UTF_16BE, REPLACEMENT];
  (!unusable.contains(&encoding)).then_some(encoding)
}

/// The value following `key`, with optional quotes, up to the first quote,
/// whitespace, `;`, `>` or `?`.
fn find_label<'a>(haystack: &'a [u8], key: &[u8]) -> Option<&'a [u8]> {
  let start = haystack.windows(key.len()).position(|w| w == key)? + key.len();
  let rest = &haystack[start..];
  let rest =
    rest.strip_prefix(b"\"").or(rest.strip_prefix(b"'")).unwrap_or(rest);
  let end = rest
    .iter()
    .position(|&b| {
      b.is_ascii_whitespace() || matches!(b, b'"' | b'\'' | b';' | b'>' | b'?')
    })
    .unwrap_or(rest.len());
  (end > 0).then_some(&rest[..end])
}

#[cfg(test)]
mod tests {
  use super::*;

  fn latin1(text: &str) -> Vec<u8> {
    text.chars().map(|c| u8::try_from(u32::from(c)).unwrap()).collect()
  }

  #[test]
  fn xml_declaration_names_the_encoding() {
    let doc = "<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?>\n\
               <html><body><p>Ça m'a coûté très cher.</p></body></html>";
    let decoded = decode_markup(&latin1(doc));
    assert_eq!(decoded.text, doc);
    assert_eq!(decoded.encoding, "windows-1252");
  }

  #[test]
  fn meta_charset_names_the_encoding() {
    let doc = "<html><head><meta http-equiv=\"Content-Type\" \
               content=\"text/html; charset=windows-1251\"></head>\
               <body><p>Привет, мир!</p></body></html>";
    let (bytes, _, unmappable) = encoding_rs::WINDOWS_1251.encode(doc);
    assert!(!unmappable);
    let decoded = decode_markup(&bytes);
    assert_eq!(decoded.text, doc);
    assert_eq!(decoded.encoding, "windows-1251");
  }

  #[test]
  fn valid_utf8_beats_a_stale_legacy_declaration() {
    let doc = "<?xml version='1.0' encoding='iso-8859-1'?><p>Ça coûte.</p>";
    let decoded = decode_markup(doc.as_bytes());
    assert_eq!(decoded.text, doc);
    assert_eq!(decoded.encoding, "UTF-8");
  }

  #[test]
  fn utf16_with_bom_ignores_the_declaration_bytes() {
    let doc = "<?xml version=\"1.0\" encoding=\"UTF-16\"?><p>Größe</p>";
    let mut bytes = vec![0xFF, 0xFE];
    bytes.extend(doc.encode_utf16().flat_map(u16::to_le_bytes));
    let decoded = decode_markup(&bytes);
    assert_eq!(decoded.text, doc);
    assert_eq!(decoded.encoding, "UTF-16LE");
  }

  #[test]
  fn a_bom_wins_over_a_contradicting_declaration() {
    // UTF-16LE BOM, document claims windows-1252.
    let doc = "<?xml version=\"1.0\" encoding=\"windows-1252\"?><p>Größe</p>";
    let mut bytes = vec![0xFF, 0xFE];
    bytes.extend(doc.encode_utf16().flat_map(u16::to_le_bytes));
    let decoded = decode_markup(&bytes);
    assert_eq!((decoded.text.as_str(), decoded.encoding), (doc, "UTF-16LE"));

    // UTF-8 BOM, <meta> claims ISO-8859-1, and the body is not even valid
    // UTF-8: still UTF-8 (lossy), never the declared charset.
    let mut bytes = b"\xEF\xBB\xBF<meta charset=iso-8859-1><p>caf".to_vec();
    bytes.extend(b"\xE9</p>");
    let decoded = decode_markup(&bytes);
    assert_eq!(decoded.encoding, "UTF-8");
    assert!(decoded.lossy && decoded.had_bom);
    assert!(decoded.text.starts_with("<meta"));
  }

  #[test]
  fn bogus_or_missing_labels_fall_back_to_detection() {
    assert_eq!(find_label(b"charset=", b"charset="), None);
    assert_eq!(declared_encoding(b"<meta charset=\"no-such\">"), None);
    assert_eq!(declared_encoding(b"<meta charset=utf-16>"), None);
    assert_eq!(declared_encoding(b"<meta charset=utf-8>"), None);
    assert_eq!(
      declared_encoding(b"<meta charset=koi8-r>"),
      Some(encoding_rs::KOI8_R)
    );
  }
}
