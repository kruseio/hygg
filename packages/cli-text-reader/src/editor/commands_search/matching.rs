// Case-insensitive matching that reports byte offsets into the ORIGINAL line.
//
// Searching `line.to_lowercase()` and using the hit offsets against `line` is
// wrong whenever lowercasing changes a character's UTF-8 length: 'İ' (2 bytes)
// lowercases to "i̇" (3 bytes), 'Ⱥ' (2 bytes) to 'ⱥ' (3 bytes). Every offset
// after such a character drifts, can land mid-character, and slicing the
// original line with it panics (issue #99). Here the line is folded one
// character at a time while recording which original character produced each
// folded byte, so a folded match maps back to a range of whole original
// characters.

use super::super::char_boundary::{ByteCol, slice_from, slice_to};

// Lowercase one character at a time. `str::to_lowercase` is context-sensitive
// (a word-final 'Σ' becomes 'ς'); folding both the line and the query the same
// context-free way keeps them comparable.
fn fold(text: &str) -> String {
  text.chars().flat_map(char::to_lowercase).collect()
}

// The folded line plus, for every folded byte, the byte span of the original
// character it came from.
struct FoldedLine {
  text: String,
  origin: Vec<(usize, usize)>,
}

impl FoldedLine {
  fn new(line: &str) -> Self {
    let mut text = String::with_capacity(line.len());
    let mut origin = Vec::with_capacity(line.len());
    for (start, c) in line.char_indices() {
      let span = (start, start + c.len_utf8());
      for lower in c.to_lowercase() {
        text.push(lower);
        origin.extend(std::iter::repeat_n(span, lower.len_utf8()));
      }
    }
    Self { text, origin }
  }

  // Map a folded `start..end` match onto whole original characters: the start
  // goes to the start of the character it falls in, the end to the end of the
  // character holding the last matched byte.
  fn to_original(&self, start: usize, end: usize) -> (usize, usize) {
    let orig_start = self.origin[start].0;
    let orig_end =
      if end > start { self.origin[end - 1].1 } else { orig_start };
    (orig_start, orig_end)
  }
}

// First case-insensitive occurrence of `query` in `line`, as a byte range of
// `line` on character boundaries.
pub(super) fn find_ci(line: &str, query: &str) -> Option<(ByteCol, ByteCol)> {
  let query = fold(query);
  if query.is_empty() {
    return None;
  }
  let folded = FoldedLine::new(line);
  let start = folded.text.find(&query)?;
  Some(original_cols(line, folded.to_original(start, start + query.len())))
}

// Last case-insensitive occurrence of `query` in `line`.
pub(super) fn rfind_ci(line: &str, query: &str) -> Option<(ByteCol, ByteCol)> {
  let query = fold(query);
  if query.is_empty() {
    return None;
  }
  let folded = FoldedLine::new(line);
  let start = folded.text.rfind(&query)?;
  Some(original_cols(line, folded.to_original(start, start + query.len())))
}

// `to_original`'s offsets come from `char_indices` of `line`, so they are its
// char boundaries; `floor` only types them.
fn original_cols(
  line: &str,
  (start, end): (usize, usize),
) -> (ByteCol, ByteCol) {
  (ByteCol::floor(line, start), ByteCol::floor(line, end))
}

// First occurrence starting at or after `from` (settled onto the line, so a
// stale column never panics).
pub(super) fn find_ci_from(
  line: &str,
  query: &str,
  from: ByteCol,
) -> Option<(ByteCol, ByteCol)> {
  let from = from.settle(line);
  // A hit in the tail is offset by where the tail starts.
  let shift = |col: ByteCol| ByteCol::floor(line, from.raw() + col.raw());
  find_ci(slice_from(line, from), query).map(|(s, e)| (shift(s), shift(e)))
}

// Last occurrence that ends at or before `before` (settled onto the line).
pub(super) fn rfind_ci_before(
  line: &str,
  query: &str,
  before: ByteCol,
) -> Option<(ByteCol, ByteCol)> {
  rfind_ci(slice_to(line, before.settle(line)), query)
}

#[cfg(test)]
mod tests {
  use super::*;

  // The raw byte range, for comparing with literal offsets.
  fn raw(found: Option<(ByteCol, ByteCol)>) -> Option<(usize, usize)> {
    found.map(|(s, e)| (s.raw(), e.raw()))
  }

  const SPANISH: &str = "La canción está aquí";

  #[test]
  fn finds_accented_text_at_byte_offsets() {
    assert_eq!(raw(find_ci(SPANISH, "ción")), Some((6, 11)));
    assert_eq!(&SPANISH[6..11], "ción");
    assert_eq!(raw(find_ci(SPANISH, "CIÓN")), Some((6, 11)));
    assert_eq!(raw(rfind_ci(SPANISH, "í")), Some((21, 23)));
  }

  #[test]
  fn length_changing_lowercase_maps_back_to_the_original_line() {
    // 'İ' is 2 bytes but lowercases to 3; 'Ⱥ' is 2 bytes and lowercases to 3.
    let line = "İstanbul Ⱥ canción €5";
    for query in ["canción", "CANCIÓN", "€5", "stanbul", "ⱥ"] {
      let (s, e) = raw(find_ci(line, query)).expect(query);
      assert_eq!(line[s..e].to_lowercase(), query.to_lowercase());
      let (s, e) = raw(rfind_ci(line, query)).expect(query);
      assert_eq!(line[s..e].to_lowercase(), query.to_lowercase());
    }
    assert_eq!(raw(find_ci(line, "canción")), Some((13, 21)));
    assert_eq!(raw(find_ci(line, "Ⱥ")), Some((10, 12)));
  }

  #[test]
  fn partial_match_inside_an_expansion_covers_the_whole_char() {
    // "i" matches the first byte of the folded "i̇"; the range must still be
    // the whole original 'İ'.
    assert_eq!(raw(find_ci("İx", "i")), Some((0, 2)));
    assert_eq!(raw(find_ci("aİ", "\u{307}")), Some((1, 3)));
  }

  #[test]
  fn from_and_before_tolerate_mid_char_columns() {
    // 'ó' occupies bytes 8..10; 9 is mid-char and floors to 8.
    assert_eq!(
      raw(find_ci_from(SPANISH, "ón", ByteCol::lit(9))),
      Some((8, 11))
    );
    assert_eq!(raw(find_ci_from(SPANISH, "ción", ByteCol::lit(7))), None);
    assert_eq!(raw(find_ci_from(SPANISH, "ción", ByteCol::lit(99))), None);
    assert_eq!(
      raw(rfind_ci_before(SPANISH, "a", ByteCol::lit(9))),
      Some((4, 5))
    );
    assert_eq!(raw(rfind_ci_before(SPANISH, "ó", ByteCol::lit(9))), None);
    assert_eq!(
      raw(rfind_ci_before(SPANISH, "ó", ByteCol::lit(10))),
      Some((8, 10))
    );
  }
}
