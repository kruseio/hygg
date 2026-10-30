//! Byte-order-mark (signature) recognition. A BOM is authoritative: when one is
//! present, the bytes are decoded as that encoding even if the content looks
//! like something else (the WHATWG "BOM sniffing" rule, which also lets a BOM
//! override an encoding declared inside an HTML/XML document).

use super::sniff::Unicode;
use super::utf7;

/// What a leading signature says about the rest of the bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Bom {
  /// UTF-8/16/32: strip the BOM and decode the body.
  Unicode(Unicode, usize),
  /// GB18030's own signature, `84 31 95 33` (U+FEFF in GB18030).
  Gb18030,
  /// UTF-7. Its BOM is not byte-aligned (the fourth character also carries
  /// bits of the next character), so the whole input is decoded and the
  /// leading U+FEFF dropped from the result.
  Utf7,
  /// A Unicode encoding scheme encoding_rs cannot decode (UTF-1, UTF-EBCDIC,
  /// SCSU, BOCU-1). Named so the caller can tell the user instead of showing
  /// mojibake.
  Unsupported(&'static str, usize),
}

/// Recognise a signature at the start of `bytes`.
///
/// Order matters where signatures share a prefix:
/// * UTF-32LE (`FF FE 00 00`) before UTF-16LE (`FF FE`). The two are genuinely
///   ambiguous for a UTF-16LE text whose first character is U+0000. That is
///   resolved toward UTF-32LE when the length is a multiple of four and at
///   least half of the 4-byte units after the BOM are valid scalar values (so a
///   UTF-32 file with a few corrupt units is still UTF-32, its bad units
///   becoming U+FFFD). UTF-16 text read as UTF-32 almost never yields valid
///   units — two ASCII characters `41 00 42 00` make 0x00420041, far above
///   U+10FFFF — so a UTF-16LE text with a leading NUL fails the test and is
///   read as UTF-16LE. A BOM-only `FF FE 00 00` is an empty UTF-32LE file.
/// * UTF-7 (`2B 2F 76` + one of `38 39 2B 2F`) is also the plain ASCII text
///   `+/v8`, `+/v9`, `+/v+` or `+/v/`, so it is UTF-7 only when the entire
///   input is well-formed UTF-7. Anything else falls through and is read as
///   ASCII/UTF-8.
pub(super) fn bom(bytes: &[u8]) -> Option<Bom> {
  const UNSUPPORTED: [(&[u8], &str); 4] = [
    (&[0xF7, 0x64, 0x4C], "UTF-1"),
    (&[0xDD, 0x73, 0x66, 0x73], "UTF-EBCDIC"),
    (&[0x0E, 0xFE, 0xFF], "SCSU"),
    (&[0xFB, 0xEE, 0x28], "BOCU-1"),
  ];
  let unicode = |form, len| Some(Bom::Unicode(form, len));
  if bytes.starts_with(&[0xFF, 0xFE, 0x00, 0x00])
    && bytes.len().is_multiple_of(4)
    && mostly_valid_utf32le(&bytes[4..])
  {
    return unicode(Unicode::Utf32Le, 4);
  }
  if bytes.starts_with(&[0x00, 0x00, 0xFE, 0xFF]) {
    return unicode(Unicode::Utf32Be, 4);
  }
  if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
    return unicode(Unicode::Utf8, 3);
  }
  if bytes.starts_with(&[0xFF, 0xFE]) {
    return unicode(Unicode::Utf16Le, 2);
  }
  if bytes.starts_with(&[0xFE, 0xFF]) {
    return unicode(Unicode::Utf16Be, 2);
  }
  if bytes.starts_with(&[0x84, 0x31, 0x95, 0x33]) {
    return Some(Bom::Gb18030);
  }
  if is_utf7_signature(bytes) && utf7::decode(bytes).is_some() {
    return Some(Bom::Utf7);
  }
  UNSUPPORTED
    .iter()
    .find(|(signature, _)| bytes.starts_with(signature))
    .map(|(signature, name)| Bom::Unsupported(name, signature.len()))
}

fn mostly_valid_utf32le(body: &[u8]) -> bool {
  let units = body.as_chunks::<4>().0;
  let valid = units
    .iter()
    .filter(|&&unit| char::from_u32(u32::from_le_bytes(unit)).is_some())
    .count();
  valid * 2 >= units.len()
}

fn is_utf7_signature(bytes: &[u8]) -> bool {
  bytes.starts_with(b"+/v")
    && matches!(bytes.get(3), Some(b'8' | b'9' | b'+' | b'/'))
}
