//! BOM edge cases: every recognised signature, BOM-vs-content conflicts, and
//! degenerate inputs (BOM only, BOM + newline, truncated or invalid units).

use super::*;

fn utf16le(text: &str) -> Vec<u8> {
  text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn decode(bytes: &[u8]) -> (String, &'static str, bool, bool) {
  let d = decode_text(bytes);
  (d.text, d.encoding, d.had_bom, d.lossy)
}

#[test]
fn bom_plus_newline_is_just_a_newline() {
  for (bom, encoding) in [
    (&b"\xEF\xBB\xBF\r\n"[..], "UTF-8"),
    (&b"\xFF\xFE\r\x00\n\x00"[..], "UTF-16LE"),
    (&b"\xFE\xFF\x00\n"[..], "UTF-16BE"),
    (&b"\xFF\xFE\x00\x00\n\x00\x00\x00"[..], "UTF-32LE"),
    (&b"\x00\x00\xFE\xFF\x00\x00\x00\n"[..], "UTF-32BE"),
    (&b"\x84\x31\x95\x33\n"[..], "gb18030"),
    (&b"+/v8-\n"[..], "UTF-7"),
  ] {
    assert_eq!(decode(bom), ("\n".into(), encoding, true, false), "{encoding}");
  }
}

#[test]
fn ff_fe_00_00_prefers_utf32le_only_when_the_body_is_valid_utf32() {
  // Valid UTF-32LE body: "Hi".
  let utf32 = b"\xFF\xFE\x00\x00H\x00\x00\x00i\x00\x00\x00";
  assert_eq!(decode(utf32).0, "Hi");
  assert_eq!(decode(utf32).1, "UTF-32LE");
  // A UTF-16LE file whose first character is U+0000, then "AB": as UTF-32LE
  // the unit 0x00420041 is out of range, so it is read as UTF-16LE.
  let mut utf16 = vec![0xFF, 0xFE];
  utf16.extend(utf16le("\0AB"));
  assert_eq!(decode(&utf16), ("\0AB".into(), "UTF-16LE", true, false));
  // Odd unit count: cannot be UTF-32 at all.
  let mut odd = vec![0xFF, 0xFE];
  odd.extend(utf16le("\0A"));
  assert_eq!(decode(&odd).1, "UTF-16LE");
}

#[test]
fn gb18030_signature_decodes_with_gb18030() {
  let (body, _, unmappable) = GB18030.encode("中文 \u{20AC} 𠀀");
  assert!(!unmappable);
  let mut bytes = vec![0x84, 0x31, 0x95, 0x33];
  bytes.extend(body.iter());
  assert_eq!(
    decode(&bytes),
    ("中文 \u{20AC} 𠀀".into(), "gb18030", true, false)
  );
}

#[test]
fn utf7_signature_requires_well_formed_utf7() {
  // "+/v8-" is the BOM alone; in "+/v9..." the fourth character already
  // carries the first bits of the next (shifted) character.
  assert_eq!(
    decode(b"+/v8-Hello, world"),
    ("Hello, world".into(), "UTF-7", true, false)
  );
  assert_eq!(decode(b"+/v9l5Wcsip4-").0, "日本語");
  // Plain ASCII that merely starts with the signature bytes stays UTF-8.
  let ascii = b"+/v8 is how a sentence might start; a + b = c";
  assert_eq!(
    decode(ascii),
    (String::from_utf8(ascii.to_vec()).unwrap(), "UTF-8", false, false)
  );
  for prefix in [&b"+/v9"[..], b"+/v+", b"+/v/"] {
    let mut bytes = prefix.to_vec();
    bytes.extend(b" not utf-7 + text");
    assert_eq!(decode(&bytes).1, "UTF-8");
  }
}

#[test]
fn undecodable_signatures_are_flagged_not_silently_garbled() {
  for (bytes, encoding) in [
    (&b"\xF7\x64\x4Chello"[..], "UTF-1"),
    (&b"\xDD\x73\x66\x73\x88\x85"[..], "UTF-EBCDIC"),
    (&b"\x0E\xFE\xFFhello"[..], "SCSU"),
    (&b"\xFB\xEE\x28hello"[..], "BOCU-1"),
  ] {
    let decoded = decode_text(bytes);
    assert_eq!(decoded.encoding, encoding);
    assert!(decoded.unsupported() && decoded.lossy && decoded.had_bom);
  }
  assert!(!decode_text(b"plain").unsupported());
}

#[test]
fn unpaired_utf16_surrogates_become_replacement_chars() {
  // BOM, lone high surrogate, 'A', lone low surrogate, then a valid pair.
  let units = [0xD800u16, 0x0041, 0xDC00, 0xD83D, 0xDE00];
  let mut bytes = vec![0xFF, 0xFE];
  bytes.extend(units.iter().flat_map(|u| u.to_le_bytes()));
  assert_eq!(
    decode(&bytes),
    ("\u{FFFD}A\u{FFFD}😀".into(), "UTF-16LE", true, true)
  );
}

#[test]
fn the_bom_wins_over_contradicting_content() {
  // UTF-16LE BOM followed by UTF-8 bytes: read as UTF-16LE regardless.
  let mut bytes = vec![0xFF, 0xFE];
  bytes.extend("héllo".as_bytes());
  let (text, encoding, had_bom, _) = decode(&bytes);
  assert_eq!((encoding, had_bom), ("UTF-16LE", true));
  let expected: Vec<u16> = "héllo"
    .as_bytes()
    .chunks(2)
    .map(|c| u16::from_le_bytes([c[0], *c.get(1).unwrap_or(&0)]))
    .collect();
  assert!(text.starts_with(&String::from_utf16_lossy(&expected[..2])));
  // UTF-8 BOM followed by Latin-1 bytes: UTF-8, lossy.
  let (text, encoding, _, lossy) = decode(b"\xEF\xBB\xBFcaf\xE9");
  assert_eq!((text.as_str(), encoding, lossy), ("caf\u{FFFD}", "UTF-8", true));
}

#[test]
fn only_a_leading_bom_is_stripped() {
  let decoded = decode_text("\u{FEFF}a\u{FEFF}b".as_bytes());
  assert_eq!(decoded.text, "a\u{FEFF}b");
  // A doubled BOM: the first is the signature, the second is content.
  let decoded = decode_text("\u{FEFF}\u{FEFF}x".as_bytes());
  assert_eq!(decoded.text, "\u{FEFF}x");
  assert_eq!(normalize_text("\u{FEFF}a\r\n\u{FEFF}b"), "a\n\u{FEFF}b");
}
