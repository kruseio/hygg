// Cursor columns, selection ends, search matches and persistent highlight
// offsets are all byte offsets into a line, because that is what `str` slicing
// and `str::find` speak. A byte offset is only usable if it lands on a
// character boundary: "canción" is eight bytes, the 'ó' occupies bytes 5..7,
// and `&line[..6]` panics. Every motion that steps through a line, and every
// renderer that slices one, goes through these helpers so the unit stays bytes
// and the value always sits on a boundary (issue #99).
//
// These are plain functions over `&str` rather than `str::floor_char_boundary`
// and friends: the workspace MSRV predates their stabilization.
//
// A character boundary keeps slicing safe, but one visible character can be
// several chars: "e\u{301}", "ที่", "👩\u{200d}💻", "🇪🇸". The cursor sits on
// the start of a grapheme cluster (extended, as `unicode-segmentation` defines
// it), and its on-screen column is the display width of the text before it,
// measured the same way cli-justify measures the lines it lays out.

mod col;
#[cfg(test)]
mod col_tests;
mod cursor_col;
mod pos;

#[cfg(test)]
pub(crate) use col::bc;
pub(crate) use col::{
  ByteCol, DisplayCol, char_starts, find_char_from, rfind_char_before,
  settle_span, slice, slice_from, slice_to,
};
pub(crate) use cursor_col::CursorCol;
pub(crate) use pos::{LineSpan, Pos, RawPos};

use unicode_segmentation::{GraphemeCursor, UnicodeSegmentation};
use unicode_width::UnicodeWidthStr;

// Clamp `idx` into the line and move it back to the start of the character it
// falls inside.
fn floor_char_boundary(line: &str, idx: usize) -> usize {
  let mut idx = idx.min(line.len());
  while idx > 0 && !line.is_char_boundary(idx) {
    idx -= 1;
  }
  idx
}

// Clamp `idx` into the line and move it forward to the end of the character it
// falls inside.
// Kept for char-level callers; cursor motion uses the grapheme forms.
#[allow(dead_code)]
fn ceil_char_boundary(line: &str, idx: usize) -> usize {
  let mut idx = idx.min(line.len());
  while idx < line.len() && !line.is_char_boundary(idx) {
    idx += 1;
  }
  idx
}

// Byte offset of the character after the one at `idx`, or `line.len()` when
// `idx` is on (or past) the last character.
fn next_char_boundary(line: &str, idx: usize) -> usize {
  let idx = floor_char_boundary(line, idx);
  slice_bytes_from(line, idx)
    .chars()
    .next()
    .map_or(line.len(), |c| idx + c.len_utf8())
}

// Byte offset of the character before the one at `idx`, or 0 at the start.
// Kept for char-level callers; cursor motion uses the grapheme forms.
#[allow(dead_code)]
fn prev_char_boundary(line: &str, idx: usize) -> usize {
  let idx = floor_char_boundary(line, idx);
  slice_bytes_to(line, idx).char_indices().next_back().map_or(0, |(i, _)| i)
}

// The character that starts at (or contains) byte `idx`.
fn char_at(line: &str, idx: usize) -> Option<char> {
  slice_bytes_from(line, idx).chars().next()
}

// Number of characters before byte `idx`. This is the on-screen column for the
// text the reader renders, which is one cell per character.
fn char_column(line: &str, idx: usize) -> usize {
  slice_bytes_to(line, idx).chars().count()
}

// Byte offset of the `col`-th character; `line.len()` past the end. The
// inverse of `char_column`, for code that walks a `Vec<char>`.
fn byte_offset_of_char(line: &str, col: usize) -> usize {
  line.char_indices().nth(col).map_or(line.len(), |(i, _)| i)
}

// Clamp a `start..end` byte range into the line and settle both ends on
// character boundaries, so `&line[start..end]` is always valid. Ends that are
// already valid do not move.
fn settle_bytes(line: &str, start: usize, end: usize) -> (usize, usize) {
  let start = floor_char_boundary(line, start);
  let end = floor_char_boundary(line, end).max(start);
  (start, end)
}

// `&line[start..end]` that cannot panic: the range is settled by
// `settle_bytes` first, so a valid range returns exactly that text and an
// invalid one is clamped. This and its two forms below are how the crate
// slices a str; raw `&s[a..b]` is linted out (`clippy::string_slice`).
fn slice_bytes(line: &str, start: usize, end: usize) -> &str {
  let (start, end) = settle_bytes(line, start, end);
  // Always `Some`: both ends are on boundaries and in order.
  line.get(start..end).unwrap_or_default()
}

// `&line[start..]` that cannot panic.
fn slice_bytes_from(line: &str, start: usize) -> &str {
  slice_bytes(line, start, line.len())
}

// `&line[..end]` that cannot panic.
fn slice_bytes_to(line: &str, end: usize) -> &str {
  slice_bytes(line, 0, end)
}

// Move `idx` back to the start of the grapheme cluster it falls inside;
// `line.len()` past the end.
fn floor_grapheme_boundary(line: &str, idx: usize) -> usize {
  let idx = floor_char_boundary(line, idx);
  let mut cursor = GraphemeCursor::new(idx, line.len(), true);
  if cursor.is_boundary(line, 0).unwrap_or(true) {
    return idx;
  }
  cursor.prev_boundary(line, 0).ok().flatten().unwrap_or(0)
}

// Byte offset of the grapheme after the one at `idx`, or `line.len()` when
// `idx` is on (or past) the last grapheme.
fn next_grapheme_boundary(line: &str, idx: usize) -> usize {
  let idx = floor_grapheme_boundary(line, idx);
  let mut cursor = GraphemeCursor::new(idx, line.len(), true);
  cursor.next_boundary(line, 0).ok().flatten().unwrap_or(line.len())
}

// Byte offset of the grapheme before the one at `idx`, or 0 at the start.
fn prev_grapheme_boundary(line: &str, idx: usize) -> usize {
  let idx = floor_grapheme_boundary(line, idx);
  let mut cursor = GraphemeCursor::new(idx, line.len(), true);
  cursor.prev_boundary(line, 0).ok().flatten().unwrap_or(0)
}

// Byte offset of the last grapheme in the line; 0 for an empty line. The
// furthest a normal-mode cursor may sit.
fn last_grapheme_start(line: &str) -> usize {
  line.grapheme_indices(true).next_back().map_or(0, |(i, _)| i)
}

// Number of grapheme clusters before byte `idx`.
fn grapheme_column(line: &str, idx: usize) -> usize {
  slice_bytes_to(line, floor_grapheme_boundary(line, idx))
    .graphemes(true)
    .count()
}

// Byte offset of the `col`-th grapheme; `line.len()` past the end. The inverse
// of `grapheme_column`.
fn byte_offset_of_grapheme(line: &str, col: usize) -> usize {
  line.grapheme_indices(true).nth(col).map_or(line.len(), |(i, _)| i)
}

// The first char of each grapheme cluster, one entry per grapheme. Motions
// that classify text by character (word, punctuation, whitespace) walk this so
// a combining mark or a ZWJ tail is never treated as a character of its own
// and an index always maps back to a grapheme start.
pub(crate) fn grapheme_base_chars(line: &str) -> Vec<char> {
  line.graphemes(true).filter_map(|g| g.chars().next()).collect()
}

// Terminal cells taken by `s`. A copy of cli-justify's `display_width`, which
// decides where the justified lines break: unicode-width per tab-separated
// span (CJK and emoji take 2, combining marks, ZWJ and zero-width chars 0),
// and 8 columns per tab.
pub(crate) fn display_width(s: &str) -> usize {
  let mut width = 0;
  for (index, span) in s.split('\t').enumerate() {
    if index > 0 {
      width += 8;
    }
    width += UnicodeWidthStr::width(span);
  }
  width
}

// On-screen column of byte `idx`: the display width of the line before the
// grapheme that contains it.
fn display_column(line: &str, idx: usize) -> usize {
  display_width(slice_bytes_to(line, floor_grapheme_boundary(line, idx)))
}

// The grapheme start whose cells cover display column `col`, clamped to the
// last grapheme. The inverse of `display_column`, used to put the cursor back
// at a remembered on-screen column on another line.
fn byte_at_display_column(line: &str, col: usize) -> usize {
  let starts: Vec<usize> =
    line.grapheme_indices(true).map(|(i, _)| i).collect();
  // Prefix widths only grow, so the last start at or before `col` is found by
  // bisection.
  let n =
    starts.partition_point(|&i| display_width(slice_bytes_to(line, i)) <= col);
  n.checked_sub(1).map_or(0, |k| starts[k])
}

#[cfg(test)]
mod tests;
