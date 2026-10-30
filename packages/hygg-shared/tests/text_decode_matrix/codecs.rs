//! The ways a test sample is turned into bytes: every Unicode form (with and
//! without a BOM) plus the legacy code pages, encoded with encoding_rs (or by
//! hand where encoding_rs has no encoder: UTF-16/32 output, true ISO-8859-1).

use encoding_rs::{Encoding, WINDOWS_1258};
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::{canonical_combining_class, compose};

#[derive(Clone, Copy, Debug)]
pub enum Codec {
  Utf8,
  Utf8Bom,
  Utf16Le {
    bom: bool,
  },
  Utf16Be {
    bom: bool,
  },
  Utf32Le {
    bom: bool,
  },
  Utf32Be {
    bom: bool,
  },
  /// Real ISO-8859-1 (one byte per code point up to U+00FF). encoding_rs maps
  /// the label to windows-1252, which agrees on every printable character.
  Latin1,
  /// A legacy encoding from the WHATWG set, encoded with encoding_rs.
  Whatwg(&'static Encoding),
  /// Vietnamese in windows-1258, which has only some precomposed letters, so
  /// the text is first rewritten into the partly decomposed form the code
  /// page can represent (base letter + combining tone mark).
  Windows1258,
}

/// Every Unicode form a sample is checked in.
pub const UNICODE: [Codec; 10] = [
  Codec::Utf8,
  Codec::Utf8Bom,
  Codec::Utf16Le { bom: true },
  Codec::Utf16Be { bom: true },
  Codec::Utf16Le { bom: false },
  Codec::Utf16Be { bom: false },
  Codec::Utf32Le { bom: true },
  Codec::Utf32Be { bom: true },
  Codec::Utf32Le { bom: false },
  Codec::Utf32Be { bom: false },
];

impl Codec {
  pub fn label(self) -> String {
    let bom = |b: bool| if b { "+BOM" } else { "" };
    match self {
      Codec::Utf8 => "UTF-8".into(),
      Codec::Utf8Bom => "UTF-8+BOM".into(),
      Codec::Utf16Le { bom: b } => format!("UTF-16LE{}", bom(b)),
      Codec::Utf16Be { bom: b } => format!("UTF-16BE{}", bom(b)),
      Codec::Utf32Le { bom: b } => format!("UTF-32LE{}", bom(b)),
      Codec::Utf32Be { bom: b } => format!("UTF-32BE{}", bom(b)),
      Codec::Latin1 => "ISO-8859-1".into(),
      Codec::Whatwg(encoding) => encoding.name().into(),
      Codec::Windows1258 => "windows-1258".into(),
    }
  }

  /// For the Unicode forms, the encoding name and BOM flag `decode_text` must
  /// report. Legacy forms are judged on the decoded text alone (several code
  /// pages decode a given text identically).
  pub fn expected_detection(self) -> Option<(&'static str, bool)> {
    match self {
      Codec::Utf8 => Some(("UTF-8", false)),
      Codec::Utf8Bom => Some(("UTF-8", true)),
      Codec::Utf16Le { bom } => Some(("UTF-16LE", bom)),
      Codec::Utf16Be { bom } => Some(("UTF-16BE", bom)),
      Codec::Utf32Le { bom } => Some(("UTF-32LE", bom)),
      Codec::Utf32Be { bom } => Some(("UTF-32BE", bom)),
      _ => None,
    }
  }

  /// The bytes for `text`, and the exact text those bytes represent (equal to
  /// `text` except for windows-1258's decomposed Vietnamese). Errors when the
  /// encoding cannot represent the text, which is a bug in the sample table.
  pub fn encode(self, text: &str) -> Result<(Vec<u8>, String), String> {
    let wide = |bom: &[u8], units: Vec<u8>| [bom.to_vec(), units].concat();
    let bytes = match self {
      Codec::Utf8 => text.as_bytes().to_vec(),
      Codec::Utf8Bom => wide(&[0xEF, 0xBB, 0xBF], text.as_bytes().to_vec()),
      Codec::Utf16Le { bom } => wide(
        if bom { &[0xFF, 0xFE] } else { &[] },
        text.encode_utf16().flat_map(u16::to_le_bytes).collect(),
      ),
      Codec::Utf16Be { bom } => wide(
        if bom { &[0xFE, 0xFF] } else { &[] },
        text.encode_utf16().flat_map(u16::to_be_bytes).collect(),
      ),
      Codec::Utf32Le { bom } => wide(
        if bom { &[0xFF, 0xFE, 0, 0] } else { &[] },
        text.chars().flat_map(|c| u32::from(c).to_le_bytes()).collect(),
      ),
      Codec::Utf32Be { bom } => wide(
        if bom { &[0, 0, 0xFE, 0xFF] } else { &[] },
        text.chars().flat_map(|c| u32::from(c).to_be_bytes()).collect(),
      ),
      Codec::Latin1 => text
        .chars()
        .map(|c| u8::try_from(u32::from(c)).map_err(|_| unmappable(self, c)))
        .collect::<Result<_, _>>()?,
      Codec::Whatwg(encoding) => whatwg(encoding, text)?,
      Codec::Windows1258 => {
        let text = to_windows_1258_form(text);
        return Ok((whatwg(WINDOWS_1258, &text)?, text));
      }
    };
    Ok((bytes, text.to_owned()))
  }
}

fn unmappable(codec: Codec, c: char) -> String {
  format!("{} cannot represent {c:?}", codec.label())
}

fn whatwg(encoding: &'static Encoding, text: &str) -> Result<Vec<u8>, String> {
  let (bytes, used, had_unmappable) = encoding.encode(text);
  if had_unmappable || used != encoding {
    return Err(format!("{} cannot represent the sample", encoding.name()));
  }
  Ok(bytes.into_owned())
}

/// Rewrite text into what windows-1258 can encode: decompose, then fold back
/// only the marks whose precomposed letter exists in the code page (â, ê, ô,
/// ă, ơ, ư, and the few toned letters it has), leaving the rest (mostly tone
/// marks) as combining characters after the base letter.
fn to_windows_1258_form(text: &str) -> String {
  let decomposed: Vec<char> = text.nfd().collect();
  let mut out = String::with_capacity(text.len() * 2);
  let mut i = 0;
  while i < decomposed.len() {
    let mut base = decomposed[i];
    i += 1;
    let mut rest = Vec::new();
    while let Some(&mark) = decomposed.get(i) {
      if canonical_combining_class(mark) == 0 {
        break;
      }
      match compose(base, mark) {
        Some(c) if !WINDOWS_1258.encode(&c.to_string()).2 => base = c,
        _ => rest.push(mark),
      }
      i += 1;
    }
    out.push(base);
    out.extend(rest);
  }
  out
}
