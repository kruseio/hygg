use super::ByteCol;

// A column on a document line. Ordered by line, then column.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Pos {
  pub(crate) line: usize,
  pub(crate) col: ByteCol,
}

// An unchecked `(line, byte column)`: what bookmarks store on disk and sync to
// other devices, laid out at another width and possibly mid-character here.
// Turned into a `Pos` with `Pos::from_raw`.
pub(crate) type RawPos = (usize, usize);

impl Pos {
  pub(crate) const fn new(line: usize, col: ByteCol) -> Self {
    Self { line, col }
  }

  // From an untrusted `(line, column)`: the column floored onto that line, or
  // 0 when the line does not exist.
  pub(crate) fn from_raw(lines: &[String], (line, col): RawPos) -> Self {
    let col =
      lines.get(line).map_or(ByteCol::ZERO, |text| ByteCol::floor(text, col));
    Self { line, col }
  }

  pub(crate) const fn to_raw(self) -> RawPos {
    (self.line, self.col.raw())
  }
}

// A byte range `start..end` of one document line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LineSpan {
  pub(crate) line: usize,
  pub(crate) start: ByteCol,
  pub(crate) end: ByteCol,
}

impl LineSpan {
  pub(crate) const fn new(line: usize, start: ByteCol, end: ByteCol) -> Self {
    Self { line, start, end }
  }
}
