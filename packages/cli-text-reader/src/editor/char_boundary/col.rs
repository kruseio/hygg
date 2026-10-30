// Typed columns. A document column is a byte offset into one line, and the
// crate also counts characters, grapheme clusters and terminal cells; mixing
// them is what issue #99 was made of. Each unit that crosses a function
// boundary gets its own type here, so a byte column passed where a character
// index is expected (or the other way round) fails to compile:
//
// - `ByteCol`: a byte offset into a line, on a char boundary, at most
//   `line.len()`. Built only by the constructors below, which check or settle
//   it against the line. There is no unchecked constructor outside tests
//   (`ByteCol::lit`): every column is built from the line it belongs to.
// - `CursorCol`: a `ByteCol` that is also the start of a grapheme cluster, the
//   only place a normal or visual cursor may sit.
// - `DisplayCol`: terminal cells, as `display_width` measures them.
// - `Pos` / `LineSpan`: a column paired with its line index.
// - `RawPos`: an unchecked `(line, column)` from disk or another device.
//
// Checking happens when a value is built. Lines can change afterwards (a PDF
// stream rebuild, a buffer switch), so a column can go stale: slicing still
// settles its bounds, and `settle` re-floors a value onto the line at hand.
// Character and grapheme indices stay plain `usize`; they only index vectors
// inside the walkers, and every conversion to or from a `ByteCol` is a named
// function.

use std::fmt;

use super::{
  byte_at_display_column, byte_offset_of_char, byte_offset_of_grapheme,
  char_column, display_column, floor_char_boundary, floor_grapheme_boundary,
  grapheme_column, last_grapheme_start, next_char_boundary,
  next_grapheme_boundary, prev_grapheme_boundary,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub(crate) struct ByteCol(usize);

impl ByteCol {
  pub(crate) const ZERO: Self = Self(0);

  // The byte offset, for absolute document offsets, persistence, logging and
  // arithmetic with other byte counts. Not for indexing characters.
  pub(crate) const fn raw(self) -> usize {
    self.0
  }

  // From an untrusted offset: clamped into the line and moved back to the
  // start of the character it falls inside.
  pub(crate) fn floor(line: &str, raw: usize) -> Self {
    Self(floor_char_boundary(line, raw))
  }

  // `Some` only if `raw` already is a boundary of the line.
  #[cfg(test)]
  pub(crate) fn checked(line: &str, raw: usize) -> Option<Self> {
    (raw <= line.len() && line.is_char_boundary(raw)).then_some(Self(raw))
  }

  // One past the last byte.
  pub(crate) fn end(line: &str) -> Self {
    Self(line.len())
  }

  // Start of the last grapheme cluster; 0 for an empty line.
  pub(crate) fn last_grapheme(line: &str) -> Self {
    Self(last_grapheme_start(line))
  }

  // The `n`-th character's start; the end of the line past it.
  pub(crate) fn from_char_index(line: &str, n: usize) -> Self {
    Self(byte_offset_of_char(line, n))
  }

  // The `n`-th grapheme cluster's start; the end of the line past it.
  pub(crate) fn from_grapheme_index(line: &str, n: usize) -> Self {
    Self(byte_offset_of_grapheme(line, n))
  }

  // The grapheme start whose cells cover display column `col`, clamped to the
  // last grapheme.
  pub(crate) fn from_display_col(line: &str, col: DisplayCol) -> Self {
    Self(byte_at_display_column(line, col.cells()))
  }

  // This column re-floored onto `line`, for a value that may be stale.
  pub(crate) fn settle(self, line: &str) -> Self {
    Self::floor(line, self.0)
  }

  pub(crate) fn next_char(self, line: &str) -> Self {
    Self(next_char_boundary(line, self.0))
  }

  pub(crate) fn next_grapheme(self, line: &str) -> Self {
    Self(next_grapheme_boundary(line, self.0))
  }

  pub(crate) fn prev_grapheme(self, line: &str) -> Self {
    Self(prev_grapheme_boundary(line, self.0))
  }

  // Start of the grapheme cluster this column falls inside.
  pub(crate) fn floor_grapheme(self, line: &str) -> Self {
    Self(floor_grapheme_boundary(line, self.0))
  }

  // Characters before this column.
  pub(crate) fn char_index(self, line: &str) -> usize {
    char_column(line, self.0)
  }

  // Grapheme clusters before this column.
  pub(crate) fn grapheme_index(self, line: &str) -> usize {
    grapheme_column(line, self.0)
  }

  // Terminal cells before the grapheme this column falls inside.
  pub(crate) fn display_col(self, line: &str) -> DisplayCol {
    DisplayCol(display_column(line, self.0))
  }

  // The character starting at (or containing) this column.
  pub(crate) fn char_at(self, line: &str) -> Option<char> {
    super::char_at(line, self.0)
  }
}

#[cfg(test)]
impl ByteCol {
  // A literal column in a test's expected value or fixture. Tests that build
  // a column from a line use `bc`, which checks it.
  pub(crate) const fn lit(raw: usize) -> Self {
    Self(raw)
  }
}

// Test helper: byte column `raw` of `line`, which must be a char boundary.
#[cfg(test)]
pub(crate) fn bc(line: &str, raw: usize) -> ByteCol {
  ByteCol::checked(line, raw)
    .unwrap_or_else(|| panic!("{raw} is not a char boundary of {line:?}"))
}

impl fmt::Display for ByteCol {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    self.0.fmt(f)
  }
}

// `start..end` settled onto `line`: both ends clamped into it and floored onto
// a character boundary, `end` no earlier than `start`. Ends that are valid do
// not move; stale or reversed ones are made safe.
pub(crate) fn settle_span(
  line: &str,
  start: ByteCol,
  end: ByteCol,
) -> (ByteCol, ByteCol) {
  let (start, end) = super::settle_bytes(line, start.0, end.0);
  (ByteCol(start), ByteCol(end))
}

// `&line[start..end]` that cannot panic: the range is settled first, so a
// valid range returns exactly that text and a stale one is clamped. This and
// its two forms below are how the crate slices a str; raw `&s[a..b]` is
// linted out (`clippy::string_slice`).
pub(crate) fn slice(line: &str, start: ByteCol, end: ByteCol) -> &str {
  super::slice_bytes(line, start.0, end.0)
}

// `&line[start..]` that cannot panic.
pub(crate) fn slice_from(line: &str, start: ByteCol) -> &str {
  super::slice_bytes_from(line, start.0)
}

// `&line[..end]` that cannot panic.
pub(crate) fn slice_to(line: &str, end: ByteCol) -> &str {
  super::slice_bytes_to(line, end.0)
}

// `line.char_indices()` with typed columns.
pub(crate) fn char_starts(
  line: &str,
) -> impl DoubleEndedIterator<Item = (ByteCol, char)> + '_ {
  line.char_indices().map(|(i, c)| (ByteCol(i), c))
}

// First `c` at or after `from`.
pub(crate) fn find_char_from(
  line: &str,
  from: ByteCol,
  c: char,
) -> Option<ByteCol> {
  let from = from.settle(line);
  slice_from(line, from).find(c).map(|i| ByteCol(from.0 + i))
}

// Last `c` that starts before `before`.
pub(crate) fn rfind_char_before(
  line: &str,
  before: ByteCol,
  c: char,
) -> Option<ByteCol> {
  slice_to(line, before).rfind(c).map(ByteCol)
}

// Terminal cells. Any count is valid, so building one is unrestricted; the
// type only keeps cells apart from bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct DisplayCol(usize);

impl DisplayCol {
  // "The end of every line", what `$` asks j/k to keep.
  pub(crate) const END: Self = Self(usize::MAX);

  #[cfg(test)]
  pub(crate) const fn new(cells: usize) -> Self {
    Self(cells)
  }

  pub(crate) const fn cells(self) -> usize {
    self.0
  }
}
