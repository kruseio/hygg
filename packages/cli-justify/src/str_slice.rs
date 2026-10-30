// Boundary-safe str slicing. `&s[a..b]` panics when an index is past the end
// or inside a multi-byte character (hygg issue #99); these clamp the range
// into `s` and floor both ends onto char boundaries instead. A valid range
// returns exactly what indexing would, so callers whose offsets come from
// `find` or `char_indices` see identical output. The crate root warns on raw
// slicing (`clippy::string_slice`), so every str slice goes through here.
//
// A private copy of cli-text-reader's `editor::char_boundary` slicing: this
// crate sits below that one in the dependency graph. Plain loops rather than
// `str::floor_char_boundary`, which postdates the workspace MSRV.

// Clamp `idx` into `s` and move it back to the start of the char it is in.
fn floor_char_boundary(s: &str, idx: usize) -> usize {
  let mut idx = idx.min(s.len());
  while idx > 0 && !s.is_char_boundary(idx) {
    idx -= 1;
  }
  idx
}

// `&s[start..end]` that cannot panic.
pub(crate) fn slice(s: &str, start: usize, end: usize) -> &str {
  let start = floor_char_boundary(s, start);
  let end = floor_char_boundary(s, end).max(start);
  // Always `Some`: both ends are on boundaries and in order.
  s.get(start..end).unwrap_or_default()
}

// `&s[start..]` that cannot panic.
pub(crate) fn slice_from(s: &str, start: usize) -> &str {
  slice(s, start, s.len())
}

// `&s[..end]` that cannot panic.
pub(crate) fn slice_to(s: &str, end: usize) -> &str {
  slice(s, 0, end)
}

#[cfg(test)]
mod tests {
  use super::*;

  // 'ó' is bytes 5..7 of the 8-byte "canción".
  const S: &str = "canción";

  #[test]
  fn valid_ranges_match_indexing() {
    let bounds: Vec<usize> =
      S.char_indices().map(|(i, _)| i).chain([S.len()]).collect();
    for &start in &bounds {
      for &end in bounds.iter().filter(|&&end| end >= start) {
        assert_eq!(slice(S, start, end), &S[start..end]);
      }
      assert_eq!(slice_from(S, start), &S[start..]);
      assert_eq!(slice_to(S, start), &S[..start]);
    }
  }

  #[test]
  fn invalid_ranges_clamp_instead_of_panicking() {
    assert_eq!(slice(S, 3, 6), "ci");
    assert_eq!(slice(S, 6, 99), "ón");
    assert_eq!(slice(S, 7, 2), "");
    assert_eq!(slice_from(S, 6), "ón");
    assert_eq!(slice_from(S, 99), "");
    assert_eq!(slice_to(S, 6), "canci");
    assert_eq!(slice_to(S, 99), S);
  }
}
