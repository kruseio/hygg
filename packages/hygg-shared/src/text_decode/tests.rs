use super::*;

fn utf16le(text: &str) -> Vec<u8> {
  text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn utf16be(text: &str) -> Vec<u8> {
  text.encode_utf16().flat_map(u16::to_be_bytes).collect()
}

fn with_prefix(prefix: &[u8], body: Vec<u8>) -> Vec<u8> {
  let mut bytes = prefix.to_vec();
  bytes.extend(body);
  bytes
}

#[test]
fn empty_input_is_empty_utf8() {
  let decoded = decode_text(b"");
  assert_eq!(decoded.text, "");
  assert_eq!(decoded.encoding, "UTF-8");
  assert!(!decoded.had_bom && !decoded.lossy);
}

#[test]
fn bom_only_inputs_decode_to_empty_text() {
  for (bom, encoding) in [
    (&[0xEF, 0xBB, 0xBF][..], "UTF-8"),
    (&[0xFF, 0xFE][..], "UTF-16LE"),
    (&[0xFE, 0xFF][..], "UTF-16BE"),
    (&[0xFF, 0xFE, 0x00, 0x00][..], "UTF-32LE"),
    (&[0x00, 0x00, 0xFE, 0xFF][..], "UTF-32BE"),
  ] {
    let decoded = decode_text(bom);
    assert_eq!(decoded.text, "", "{encoding}");
    assert_eq!(decoded.encoding, encoding);
    assert!(decoded.had_bom && !decoded.lossy, "{encoding}");
  }
}

#[test]
fn utf8_bom_is_stripped() {
  let decoded = decode_text(b"\xEF\xBB\xBFcaf\xC3\xA9");
  assert_eq!(decoded.text, "café");
  assert_eq!(decoded.encoding, "UTF-8");
  assert!(decoded.had_bom);
}

#[test]
fn utf16_boms_are_honoured_and_stripped() {
  let le = decode_text(&with_prefix(&[0xFF, 0xFE], utf16le("¿Qué tal?")));
  assert_eq!((le.text.as_str(), le.encoding), ("¿Qué tal?", "UTF-16LE"));
  let be = decode_text(&with_prefix(&[0xFE, 0xFF], utf16be("Grüße 👋")));
  assert_eq!((be.text.as_str(), be.encoding), ("Grüße 👋", "UTF-16BE"));
  assert!(le.had_bom && be.had_bom && !le.lossy && !be.lossy);
}

#[test]
fn utf32_invalid_units_and_truncation_become_replacement_chars() {
  let mut bytes = vec![0xFF, 0xFE, 0x00, 0x00];
  bytes.extend(0x41u32.to_le_bytes());
  bytes.extend(0xD800u32.to_le_bytes()); // surrogate
  bytes.extend(0x0011_0000u32.to_le_bytes()); // above U+10FFFF
  bytes.extend(0x1F600u32.to_le_bytes());
  let decoded = decode_text(&bytes);
  assert_eq!(decoded.text, "A\u{FFFD}\u{FFFD}😀");
  assert!(decoded.lossy);

  let mut be = vec![0x00, 0x00, 0xFE, 0xFF, 0x00, 0x00, 0x00, 0x42, 0x00];
  be.truncate(9);
  let decoded = decode_text(&be);
  assert_eq!(decoded.text, "B\u{FFFD}");
  assert_eq!(decoded.encoding, "UTF-32BE");
  assert!(decoded.lossy);
}

#[test]
fn truncated_utf16_is_lossy_not_a_failure() {
  let mut bytes = with_prefix(&[0xFF, 0xFE], utf16le("abc"));
  bytes.push(0x64);
  let decoded = decode_text(&bytes);
  assert_eq!(decoded.text, "abc\u{FFFD}");
  assert!(decoded.lossy);
}

#[test]
fn bomless_utf16_and_utf32_are_recognised() {
  let text = "Hello, world.\nThis has no byte-order mark at all.\n";
  assert_eq!(decode_text(&utf16le(text)).encoding, "UTF-16LE");
  assert_eq!(decode_text(&utf16le(text)).text, text);
  assert_eq!(decode_text(&utf16be(text)).encoding, "UTF-16BE");
  assert_eq!(decode_text(&utf16be(text)).text, text);

  let utf32: Vec<u8> =
    text.chars().flat_map(|c| u32::from(c).to_be_bytes()).collect();
  let decoded = decode_text(&utf32);
  assert_eq!((decoded.text.as_str(), decoded.encoding), (text, "UTF-32BE"));
  assert!(!decoded.had_bom);
}

#[test]
fn utf8_with_a_stray_nul_stays_utf8() {
  let text = "A log line\0 with one NUL byte in the middle of a long line.";
  let decoded = decode_text(text.as_bytes());
  assert_eq!(decoded.text, text);
  assert_eq!(decoded.encoding, "UTF-8");
}

#[test]
fn line_endings_are_normalised() {
  let decoded = decode_text(b"one\r\ntwo\rthree\n\r\nfour\r");
  assert_eq!(decoded.text, "one\ntwo\nthree\n\nfour\n");
  let crlf = with_prefix(&[0xFF, 0xFE], utf16le("a\r\n\r\nb"));
  assert_eq!(decode_text(&crlf).text, "a\n\nb");
}

#[test]
fn ansi_escapes_stay_utf8() {
  let text = "\x1b[1mbold\x1b[0m and \x1b(B is not ISO-2022 when alone";
  let decoded = decode_text(text.as_bytes());
  assert_eq!(decoded.encoding, "UTF-8");
  assert_eq!(decoded.text, text);
}

#[test]
fn iso_2022_jp_is_detected_despite_being_valid_ascii() {
  let text = "日本語の文章です。";
  let (bytes, _, unmappable) = ISO_2022_JP.encode(text);
  assert!(!unmappable);
  assert!(std::str::from_utf8(&bytes).is_ok());
  let decoded = decode_text(&bytes);
  assert_eq!((decoded.text.as_str(), decoded.encoding), (text, "ISO-2022-JP"));
}

#[test]
fn arbitrary_bytes_never_panic() {
  // A crude xorshift sweep over random-looking inputs of many lengths,
  // including NUL-heavy and high-bit-heavy ones.
  let mut state = 0x9E37_79B9_7F4A_7C15u64;
  for len in 0..512 {
    let bytes: Vec<u8> = (0..len)
      .map(|i| {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        if i % 3 == 0 { 0 } else { state as u8 }
      })
      .collect();
    let decoded = decode_text(&bytes);
    assert!(!decoded.text.contains('\r'));
    let _ = markup::decode_markup(&bytes);
  }
}
