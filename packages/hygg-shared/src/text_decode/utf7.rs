//! A strict UTF-7 (RFC 2152) decoder. encoding_rs omits UTF-7 (the WHATWG
//! standard forbids it), and it is only ever used here for input that starts
//! with the UTF-7 signature, so strictness is the point: anything malformed
//! returns `None` and the caller treats the bytes as ASCII/UTF-8 instead.

/// Decode `bytes` as UTF-7, or `None` if they are not well-formed UTF-7: a
/// non-ASCII byte, a `+` not followed by base64 or `-`, a shifted run whose
/// leftover bits are not zero padding of under six bits, or an unpaired
/// surrogate.
pub(super) fn decode(bytes: &[u8]) -> Option<String> {
  let mut out = String::with_capacity(bytes.len());
  let mut i = 0;
  while i < bytes.len() {
    let byte = bytes[i];
    if !byte.is_ascii() {
      return None;
    }
    i += 1;
    if byte != b'+' {
      out.push(char::from(byte));
      continue;
    }
    if bytes.get(i) == Some(&b'-') {
      out.push('+');
      i += 1;
      continue;
    }
    let (units, end) = shifted_run(bytes, i)?;
    out.push_str(&String::from_utf16(&units).ok()?);
    // An explicit `-` closes the run and is absorbed; any other terminator is
    // an ordinary direct character, handled on the next iteration.
    i = if bytes.get(end) == Some(&b'-') { end + 1 } else { end };
  }
  Some(out)
}

/// Decode the modified-base64 run starting at `start` into UTF-16 code units,
/// returning them and the index just past the run.
fn shifted_run(bytes: &[u8], start: usize) -> Option<(Vec<u16>, usize)> {
  let mut units = Vec::new();
  let (mut acc, mut bits) = (0u32, 0u32);
  let mut i = start;
  while let Some(value) = bytes.get(i).and_then(|&b| base64_value(b)) {
    acc = (acc << 6) | value;
    bits += 6;
    if bits >= 16 {
      bits -= 16;
      units.push((acc >> bits) as u16);
      acc &= (1 << bits) - 1;
    }
    i += 1;
  }
  let well_formed = i > start && bits < 6 && acc == 0;
  well_formed.then_some((units, i))
}

fn base64_value(byte: u8) -> Option<u32> {
  let value = match byte {
    b'A'..=b'Z' => byte - b'A',
    b'a'..=b'z' => byte - b'a' + 26,
    b'0'..=b'9' => byte - b'0' + 52,
    b'+' => 62,
    b'/' => 63,
    _ => return None,
  };
  Some(u32::from(value))
}

#[cfg(test)]
mod tests {
  use super::decode;

  #[test]
  fn decodes_rfc_2152_examples() {
    assert_eq!(
      decode(b"Hi Mom -+Jjo--!").as_deref(),
      Some("Hi Mom -\u{263A}-!")
    );
    assert_eq!(decode(b"+ZeVnLIqe-").as_deref(), Some("日本語"));
    assert_eq!(decode(b"A+ImIDkQ.").as_deref(), Some("A\u{2262}\u{391}."));
    assert_eq!(decode(b"1 +- 1 = 2").as_deref(), Some("1 + 1 = 2"));
    assert_eq!(decode(b"+/v8-hello").as_deref(), Some("\u{FEFF}hello"));
  }

  #[test]
  fn rejects_malformed_input() {
    assert_eq!(decode(b"a + b"), None); // `+` then a non-base64 byte
    assert_eq!(decode(b"C++ is fun"), None); // `++` leaves 6 stray bits
    assert_eq!(decode(b"+/v"), None); // 12 bits: no whole unit, not padding
    assert_eq!(decode(b"+2D0-"), None); // lone high surrogate U+D83D
    assert_eq!(decode("caf\u{e9}".as_bytes()), None); // not 7-bit
  }
}
