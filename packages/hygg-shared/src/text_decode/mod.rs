//! Decoding of arbitrary text bytes into a Rust `String`.
//!
//! Plain-text files and piped input arrive in whatever encoding the author's
//! editor or OS used: UTF-8 (with or without a BOM), UTF-16 from Windows
//! Notepad, or a legacy code page such as Windows-1252, KOI8-R or Shift_JIS.
//! [`decode_text`] turns any of them into UTF-8 and never fails: bytes that
//! cannot be decoded become U+FFFD and the result is flagged `lossy`.
//!
//! Detection order:
//! 1. A byte-order mark is authoritative and is stripped: UTF-8, UTF-16LE/BE,
//!    UTF-32LE/BE, GB18030 and (when the whole input is well-formed) UTF-7. The
//!    signatures of UTF-1, UTF-EBCDIC, SCSU and BOCU-1 are recognised but those
//!    schemes cannot be decoded; see [`DecodedText::unsupported`].
//! 2. BOM-less UTF-32 / UTF-16, recognised by where the NUL bytes fall.
//! 3. Valid UTF-8 is taken as is (pure ASCII included), except 7-bit input
//!    carrying ISO-2022-JP escape sequences.
//! 4. Anything else goes to `chardetng`, Mozilla's legacy-encoding detector,
//!    and is decoded with `encoding_rs` (Windows-125x, ISO-8859-x, KOI8-R/U,
//!    Shift_JIS, EUC-JP, ISO-2022-JP, GBK/GB18030, Big5, EUC-KR, ...).
//!
//! The decoded text also has its line endings normalised (CRLF and lone CR
//! become LF): the justifier splits paragraphs on `"\n\n"`, which a CRLF file
//! would otherwise never contain.
//!
//! Pure Rust with no I/O, so it builds for `wasm32-unknown-unknown` too.

mod bom;
mod markup;
mod sniff;
mod utf7;

use bom::Bom;
use encoding_rs::{Encoding, GB18030, ISO_2022_JP, UTF_8, UTF_16BE, UTF_16LE};
pub use markup::decode_markup;
use sniff::Unicode;

/// The result of [`decode_text`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedText {
  /// The decoded text, BOM stripped, line endings normalised to `\n`.
  pub text: String,
  /// WHATWG name of the encoding the bytes were decoded as (`"UTF-8"`,
  /// `"UTF-16LE"`, `"windows-1252"`, `"Shift_JIS"`, ...), or `"UTF-32LE"` /
  /// `"UTF-32BE"`, which the WHATWG standard does not define.
  pub encoding: &'static str,
  /// Whether the input started with a byte-order mark (which was removed).
  pub had_bom: bool,
  /// Whether some bytes were malformed in the chosen encoding and were
  /// replaced with U+FFFD. Always set for an [unsupported](Self::unsupported)
  /// encoding.
  pub lossy: bool,
}

/// Signature-identified encodings that cannot be decoded (encoding_rs does not
/// implement them, and they are too rare to hand-roll).
const UNSUPPORTED: [&str; 4] = ["UTF-1", "UTF-EBCDIC", "SCSU", "BOCU-1"];

impl DecodedText {
  /// True when the input carried the signature of an encoding that cannot be
  /// decoded (UTF-1, UTF-EBCDIC, SCSU, BOCU-1). `text` is then only a lossy
  /// UTF-8 reading of the bytes after the signature; callers should tell the
  /// user the file is in `encoding` rather than present it as the document.
  pub fn unsupported(&self) -> bool {
    UNSUPPORTED.contains(&self.encoding)
  }
}

/// Decode `bytes` of unknown encoding into text. Never fails; see the module
/// docs for the detection order.
pub fn decode_text(bytes: &[u8]) -> DecodedText {
  decode_with_declared(bytes, None)
}

/// Shared body of [`decode_text`] and [`decode_markup`]: `declared` is a
/// charset the document itself names, consulted after the Unicode checks and
/// before statistical detection.
fn decode_with_declared(
  bytes: &[u8],
  declared: Option<&'static Encoding>,
) -> DecodedText {
  let mut decoded = decode_raw(bytes, declared);
  normalize_newlines(&mut decoded.text);
  decoded
}

fn decode_raw(
  bytes: &[u8],
  declared: Option<&'static Encoding>,
) -> DecodedText {
  if let Some(bom) = bom::bom(bytes) {
    return decode_bom(bom, bytes);
  }
  if let Some(unicode) = sniff::bomless_wide(bytes) {
    return decode_unicode(unicode, bytes);
  }
  if let Ok(text) = std::str::from_utf8(bytes) {
    if sniff::has_iso_2022_escape(bytes) && detect(bytes, true) == ISO_2022_JP {
      return decode_with(ISO_2022_JP, bytes);
    }
    return DecodedText {
      text: text.to_owned(),
      encoding: UTF_8.name(),
      had_bom: false,
      lossy: false,
    };
  }
  let encoding = declared.unwrap_or_else(|| detect(bytes, false));
  decode_with(encoding, bytes)
}

/// Ask chardetng for the most likely encoding of the whole input.
fn detect(bytes: &[u8], allow_utf8: bool) -> &'static Encoding {
  use chardetng::{Iso2022JpDetection, Utf8Detection};
  // chardetng 0.1 always considered ISO-2022-JP; 1.0 made it opt-in.
  let mut detector =
    chardetng::EncodingDetector::new(Iso2022JpDetection::Allow);
  detector.feed(bytes, true);
  let utf8 =
    if allow_utf8 { Utf8Detection::Allow } else { Utf8Detection::Deny };
  detector.guess(None, utf8)
}

fn decode_with(encoding: &'static Encoding, bytes: &[u8]) -> DecodedText {
  let (text, lossy) = encoding.decode_without_bom_handling(bytes);
  DecodedText {
    text: text.into_owned(),
    encoding: encoding.name(),
    had_bom: false,
    lossy,
  }
}

fn decode_bom(bom: Bom, bytes: &[u8]) -> DecodedText {
  let mut decoded = match bom {
    Bom::Unicode(unicode, len) => decode_unicode(unicode, &bytes[len..]),
    Bom::Gb18030 => decode_with(GB18030, &bytes[4..]),
    Bom::Utf7 => {
      let text = utf7::decode(bytes).unwrap_or_default();
      let text = text.strip_prefix('\u{FEFF}').unwrap_or(&text).to_owned();
      DecodedText { text, encoding: "UTF-7", had_bom: false, lossy: false }
    }
    Bom::Unsupported(encoding, len) => {
      let text = String::from_utf8_lossy(&bytes[len..]).into_owned();
      DecodedText { text, encoding, had_bom: false, lossy: true }
    }
  };
  decoded.had_bom = true;
  decoded
}

fn decode_unicode(unicode: Unicode, body: &[u8]) -> DecodedText {
  match unicode {
    Unicode::Utf8 => decode_with(UTF_8, body),
    Unicode::Utf16Le => decode_with(UTF_16LE, body),
    Unicode::Utf16Be => decode_with(UTF_16BE, body),
    Unicode::Utf32Le => decode_utf32(body, u32::from_le_bytes, "UTF-32LE"),
    Unicode::Utf32Be => decode_utf32(body, u32::from_be_bytes, "UTF-32BE"),
  }
}

/// encoding_rs has no UTF-32 (the WHATWG standard drops it), so decode it by
/// hand: each 4-byte unit is one scalar value, anything else (surrogates,
/// values above U+10FFFF, a truncated tail) becomes U+FFFD.
fn decode_utf32(
  body: &[u8],
  unit: fn([u8; 4]) -> u32,
  encoding: &'static str,
) -> DecodedText {
  let (units, tail) = body.as_chunks::<4>();
  let truncated = !tail.is_empty();
  let mut lossy = truncated;
  let mut text = String::with_capacity(units.len());
  for &bytes in units {
    let value = unit(bytes);
    text.push(char::from_u32(value).unwrap_or_else(|| {
      lossy = true;
      char::REPLACEMENT_CHARACTER
    }));
  }
  if truncated {
    text.push(char::REPLACEMENT_CHARACTER);
  }
  DecodedText { text, encoding, had_bom: false, lossy }
}

/// Tidy text that is already a Rust string (the output of another converter,
/// such as an EPUB or pandoc extraction) the way [`decode_text`] tidies decoded
/// bytes: drop one leading U+FEFF (a BOM that survived into the text) and
/// normalise line endings to LF. A U+FEFF anywhere else is a legitimate
/// zero-width no-break space and is kept.
pub fn normalize_text(text: &str) -> String {
  let mut text = text.strip_prefix('\u{FEFF}').unwrap_or(text).to_owned();
  normalize_newlines(&mut text);
  text
}

/// Rewrite CRLF and lone CR line endings as LF, in place.
fn normalize_newlines(text: &mut String) {
  if !text.contains('\r') {
    return;
  }
  let mut out = String::with_capacity(text.len());
  let mut chars = text.chars().peekable();
  while let Some(ch) = chars.next() {
    if ch == '\r' {
      if chars.peek() == Some(&'\n') {
        chars.next();
      }
      out.push('\n');
    } else {
      out.push(ch);
    }
  }
  *text = out;
}

#[cfg(test)]
mod bom_tests;
#[cfg(test)]
mod tests;
