//! Byte-level checks that identify Unicode encodings without a byte-order mark
//! before any statistical detection runs: BOM-less UTF-32/UTF-16, and
//! ISO-2022-JP escapes hiding inside otherwise valid (7-bit) UTF-8.

/// A Unicode encoding form recognised from the raw bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Unicode {
  Utf8,
  Utf16Le,
  Utf16Be,
  Utf32Le,
  Utf32Be,
}

/// How many leading bytes the BOM-less heuristics look at. Plenty to see the
/// NUL pattern of wide text, and keeps the scan cheap on large files.
const SAMPLE_LEN: usize = 64 * 1024;

/// Recognise UTF-32 or UTF-16 written without a BOM. Legacy 8-bit and UTF-8
/// text practically never contains NUL bytes, whereas wide text does: every
/// ASCII character (space, newline, digits, punctuation) has a zero high byte.
pub(super) fn bomless_wide(bytes: &[u8]) -> Option<Unicode> {
  let sample = &bytes[..bytes.len().min(SAMPLE_LEN)];
  if !sample.contains(&0) {
    return None;
  }
  bomless_utf32(bytes, sample).or_else(|| bomless_utf16(bytes, sample))
}

/// UTF-32 without a BOM: every 4-byte unit of the sample is a non-NUL scalar
/// value in exactly one byte order.
fn bomless_utf32(bytes: &[u8], sample: &[u8]) -> Option<Unicode> {
  if bytes.len() < 8 || !bytes.len().is_multiple_of(4) {
    return None;
  }
  let units_valid = |unit: fn([u8; 4]) -> u32| utf32_units_valid(sample, unit);
  match (units_valid(u32::from_le_bytes), units_valid(u32::from_be_bytes)) {
    (true, false) => Some(Unicode::Utf32Le),
    (false, true) => Some(Unicode::Utf32Be),
    _ => None,
  }
}

/// Whether every whole 4-byte unit of `bytes` is a non-NUL Unicode scalar
/// value.
fn utf32_units_valid(bytes: &[u8], unit: fn([u8; 4]) -> u32) -> bool {
  bytes.as_chunks::<4>().0.iter().all(|&bytes| {
    let value = unit(bytes);
    value != 0 && char::from_u32(value).is_some()
  })
}

/// UTF-16 without a BOM (the input is known to contain a NUL byte).
///
/// The byte position holding the high byte of each code unit is found first:
/// either the one where the NULs clearly cluster (ASCII characters have a zero
/// high byte), or else the one with markedly fewer distinct values — text keeps
/// to a few Unicode blocks, so its high bytes repeat (0x0E for all of Thai,
/// 0x4E..0x9F for CJK) while its low bytes spread out. Legacy 8-bit text shows
/// neither pattern. Input that is already valid UTF-8 (Thai in UTF-16 is all
/// bytes below 0x80!) must also be control-character heavy as UTF-8, which
/// real UTF-8 text never is. Finally the sample must be well-formed UTF-16 in
/// the chosen byte order.
fn bomless_utf16(bytes: &[u8], sample: &[u8]) -> Option<Unicode> {
  let mut nuls = [0usize; 2];
  let mut seen = [[false; 256]; 2];
  for pair in sample.as_chunks::<2>().0 {
    for (position, &byte) in pair.iter().enumerate() {
      nuls[position] += usize::from(byte == 0);
      seen[position][usize::from(byte)] = true;
    }
  }
  let distinct = seen.map(|s| s.iter().filter(|&&b| b).count());
  let clearly_fewer = |a: usize, b: usize| a * 4 <= b * 3;
  let high = if nuls[1] > nuls[0] * 4 {
    1
  } else if nuls[0] > nuls[1] * 4 {
    0
  } else if clearly_fewer(distinct[1], distinct[0]) {
    1
  } else if clearly_fewer(distinct[0], distinct[1]) {
    0
  } else {
    return None;
  };
  if std::str::from_utf8(bytes).is_ok() && !control_heavy(sample) {
    return None;
  }
  let (unicode, to_unit): (_, fn([u8; 2]) -> u16) = if high == 1 {
    (Unicode::Utf16Le, u16::from_le_bytes)
  } else {
    (Unicode::Utf16Be, u16::from_be_bytes)
  };
  let truncated = sample.len() < bytes.len();
  well_formed_utf16(sample, to_unit, truncated).then_some(unicode)
}

/// At least a tenth of the bytes are C0 controls other than the whitespace
/// ones (tab, LF, FF, CR) and ESC.
fn control_heavy(sample: &[u8]) -> bool {
  let controls = sample
    .iter()
    .filter(|&&b| b < 0x20 && !matches!(b, b'\t' | b'\n' | 0x0C | b'\r' | 0x1B))
    .count();
  controls * 10 >= sample.len()
}

/// No unpaired surrogates. A high surrogate at the very end is tolerated when
/// the sample was cut from a longer input (its pair lies past the cut).
fn well_formed_utf16(
  sample: &[u8],
  to_unit: fn([u8; 2]) -> u16,
  truncated: bool,
) -> bool {
  let mut units = sample.as_chunks::<2>().0.iter().map(|&pair| to_unit(pair));
  while let Some(unit) = units.next() {
    match unit {
      0xD800..=0xDBFF => match units.next() {
        Some(0xDC00..=0xDFFF) => {}
        Some(_) => return false,
        None => return truncated,
      },
      0xDC00..=0xDFFF => return false,
      _ => {}
    }
  }
  true
}

/// Whether the bytes contain an ISO-2022-JP designation escape (`ESC $` or
/// `ESC (`). ANSI terminal sequences (`ESC [`) do not count.
pub(super) fn has_iso_2022_escape(bytes: &[u8]) -> bool {
  bytes.windows(2).any(|w| w[0] == 0x1B && matches!(w[1], b'$' | b'('))
}
