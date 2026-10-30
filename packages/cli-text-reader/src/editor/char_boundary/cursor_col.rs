use std::fmt;

use super::{ByteCol, DisplayCol, char_starts};

// Where a normal or visual cursor sits: the start of a grapheme cluster, so it
// never splits a visible character. Wraps a `ByteCol` and converts to one for
// free; the way back goes through `floor`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub(crate) struct CursorCol(ByteCol);

impl CursorCol {
  pub(crate) const ZERO: Self = Self(ByteCol::ZERO);

  // The start of the grapheme `col` falls inside (of `line`, clamped to its
  // end).
  pub(crate) fn floor(line: &str, col: ByteCol) -> Self {
    Self(col.floor_grapheme(line))
  }

  // As `floor`, and no further than the last grapheme: where a normal-mode
  // cursor may rest.
  pub(crate) fn clamped(line: &str, col: ByteCol) -> Self {
    Self(col.floor_grapheme(line).min(ByteCol::last_grapheme(line)))
  }

  pub(crate) fn last(line: &str) -> Self {
    Self(ByteCol::last_grapheme(line))
  }

  // The first grapheme that does not start with whitespace, or 0 on a blank
  // line: where `^` and line jumps put the cursor.
  pub(crate) fn first_non_blank(line: &str) -> Self {
    let col = char_starts(line)
      .find(|&(_, c)| !c.is_whitespace())
      .map_or(ByteCol::ZERO, |(col, _)| col);
    Self::floor(line, col)
  }

  pub(crate) fn from_display_col(line: &str, col: DisplayCol) -> Self {
    Self(ByteCol::from_display_col(line, col))
  }

  pub(crate) fn next(self, line: &str) -> Self {
    Self(self.0.next_grapheme(line))
  }

  pub(crate) fn prev(self, line: &str) -> Self {
    Self(self.0.prev_grapheme(line))
  }

  pub(crate) const fn col(self) -> ByteCol {
    self.0
  }

  pub(crate) const fn raw(self) -> usize {
    self.0.raw()
  }

  pub(crate) fn display_col(self, line: &str) -> DisplayCol {
    self.0.display_col(line)
  }
}

#[cfg(test)]
impl CursorCol {
  // A literal cursor column in a test: an expected value, or a deliberately
  // stale or mid-character one to exercise the code that settles it.
  pub(crate) const fn lit(raw: usize) -> Self {
    Self(ByteCol::lit(raw))
  }
}

impl From<CursorCol> for ByteCol {
  fn from(col: CursorCol) -> Self {
    col.0
  }
}

impl fmt::Display for CursorCol {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    self.0.fmt(f)
  }
}
