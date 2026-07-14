//! Reader text-selection geometry.
//!
//! The reader draws each line as a plain widget on a canvas, so there is no
//! built-in selection — we build one. Because the column is **monospace**,
//! mapping a pointer position to a `(line, column)` anchor is exact arithmetic
//! (one advance wide, one line-height tall) with no per-glyph font metrics.
//! These are pure functions (no toolkit state) so the update handler and the
//! view can share them.

use crate::layout;
use crate::model::Book;

/// A caret position in the document: `(line, column)` counted in characters.
pub type Pos = (usize, usize);

/// Map a pointer at `(x, y)` — relative to the scroll viewport's top-left — to
/// a `(line, column)`. `scroll_y` is the current scroll offset, `width` the
/// viewport width, `font`/`lh` the fitted glyph size and line height. The whole
/// `col`-wide column is centered as one block (like the reader), so every line
/// shares the same left margin — it does not depend on the line's own length.
pub fn locate(
  book: &Book,
  x: f32,
  y: f32,
  scroll_y: f32,
  width: f32,
  font: f64,
  lh: f32,
) -> Pos {
  let content_y = (scroll_y + y).max(0.0) as f64;
  let total = book.lines.len();
  let line = ((content_y / lh.max(1.0) as f64).floor() as usize)
    .min(total.saturating_sub(1));
  let adv = layout::char_advance(font);
  let len = book.lines.get(line).map_or(0, |l| l.chars().count());
  // The `col`-wide column is centered as one block (see the reader), so every
  // line shares this left margin — derived from `col`, not the line's own
  // width.
  let col_w = layout::block_width(book.col, font, width) as f64;
  let left = (width as f64 - col_w) / 2.0;
  let col = if adv > 0.0 {
    ((x as f64 - left) / adv).round().max(0.0) as usize
  } else {
    0
  };
  (line, col.min(len))
}

/// Normalize an anchor/cursor pair into `(start, end)` with `start <= end`, or
/// `None` when they coincide (a click, not a drag — nothing selected).
pub fn normalize(anchor: Pos, cursor: Pos) -> Option<(Pos, Pos)> {
  if anchor == cursor {
    None
  } else if anchor <= cursor {
    Some((anchor, cursor))
  } else {
    Some((cursor, anchor))
  }
}

/// The selected `[start, end)` column range on line `i` (a line `len` chars
/// long), or `None` when the line is outside the selection or the range is
/// empty.
pub fn cols_on_line(
  sel: (Pos, Pos),
  i: usize,
  len: usize,
) -> Option<(usize, usize)> {
  let ((sl, sc), (el, ec)) = sel;
  if i < sl || i > el {
    return None;
  }
  let s = if i == sl { sc } else { 0 };
  let e = if i == el { ec } else { len };
  let (s, e) = (s.min(len), e.min(len));
  (s < e).then_some((s, e))
}

/// The `[start, end)` char columns of the "word" around `col` — browser-style
/// double-click selection based on **Unicode text segmentation (UAX #29** word
/// boundaries), plus the common browser heuristic that a run of consecutive
/// punctuation/symbols selects together. So in `http://10.121.121.166:3032`,
/// `http` / `://` / `10.121.121.166` (digits joined by `.`) / `:` / `3032` each
/// select on their own.
pub fn word_bounds(line: &str, col: usize) -> (usize, usize) {
  use unicode_segmentation::UnicodeSegmentation;
  // UAX #29 word segments as (start, end) char indices, merging adjacent
  // punctuation runs (segments that are neither alphanumeric nor whitespace).
  let mut segs: Vec<(usize, usize)> = Vec::new();
  let mut start = 0;
  let mut prev_punct = false;
  for piece in line.split_word_bounds() {
    let end = start + piece.chars().count();
    let space = piece.chars().all(char::is_whitespace);
    let word = piece.chars().any(|c| c.is_alphanumeric() || c == '_');
    let punct = !space && !word;
    if punct && prev_punct {
      segs.last_mut().unwrap().1 = end;
    } else {
      segs.push((start, end));
    }
    prev_punct = punct;
    start = end;
  }
  // `start` is now the total length; clamp `col` into the text and find its
  // segment.
  let col = col.min(start.saturating_sub(1));
  segs.into_iter().find(|&(s, e)| col >= s && col < e).unwrap_or((0, 0))
}

/// Extract the selected text, lines joined by `\n`.
pub fn extract(book: &Book, sel: (Pos, Pos)) -> String {
  let ((sl, _), (el, _)) = sel;
  let mut out = String::new();
  for i in sl..=el {
    let chars: Vec<char> =
      book.lines.get(i).map(|l| l.chars().collect()).unwrap_or_default();
    if let Some((s, e)) = cols_on_line(sel, i, chars.len()) {
      out.extend(&chars[s..e]);
    }
    if i != el {
      out.push('\n');
    }
  }
  out
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::model::{Book, LineKind};

  fn book(lines: &[&str]) -> Book {
    let lines: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
    Book {
      id: "id".into(),
      title: "t".into(),
      format: "txt".into(),
      col: 20,
      kinds: vec![LineKind::Text; lines.len()],
      lines,
      size_bytes: 0,
      added_at: 0.0,
      page_starts: vec![],
    }
  }

  #[test]
  fn normalize_orders_and_rejects_empty() {
    assert_eq!(normalize((1, 2), (1, 2)), None);
    assert_eq!(normalize((3, 1), (1, 5)), Some(((1, 5), (3, 1))));
    assert_eq!(normalize((1, 5), (3, 1)), Some(((1, 5), (3, 1))));
  }

  #[test]
  fn cols_clip_first_and_last_lines() {
    let sel = ((1, 2), (3, 4));
    assert_eq!(cols_on_line(sel, 0, 10), None); // above
    assert_eq!(cols_on_line(sel, 1, 10), Some((2, 10))); // first: from col
    assert_eq!(cols_on_line(sel, 2, 10), Some((0, 10))); // middle: whole line
    assert_eq!(cols_on_line(sel, 3, 10), Some((0, 4))); // last: to col
    assert_eq!(cols_on_line(sel, 4, 10), None); // below
    assert_eq!(cols_on_line(((1, 3), (1, 3)), 1, 10), None); // empty range
  }

  #[test]
  fn extract_single_and_multi_line() {
    let b = book(&["hello world", "second line", "third row"]);
    // Within one line.
    assert_eq!(extract(&b, ((0, 0), (0, 5))), "hello");
    // Across lines: tail of first, all of middle, head of last.
    assert_eq!(extract(&b, ((0, 6), (2, 5))), "world\nsecond line\nthird");
  }

  #[test]
  fn word_bounds_selects_the_word_or_the_gap() {
    let s = "the  quick brown";
    // Inside "quick" (cols 5..10) → the whole word.
    assert_eq!(word_bounds(s, 6), (5, 10));
    // On a space in the double gap (cols 3..5) → the run of whitespace.
    assert_eq!(word_bounds(s, 4), (3, 5));
    // First word.
    assert_eq!(word_bounds(s, 0), (0, 3));
    // Past the end clamps into the last word ("brown").
    assert_eq!(word_bounds(s, 99), (11, 16));
    assert_eq!(word_bounds("", 0), (0, 0));
  }

  #[test]
  fn word_bounds_url_segments_like_a_browser() {
    // UAX #29 + punctuation grouping: http / :// / 10.121.121.166 / : / 3032.
    let u = "http://10.121.121.166:3032";
    assert_eq!(word_bounds(u, 1), (0, 4)); // "http"
    assert_eq!(word_bounds(u, 5), (4, 7)); // "://"
    assert_eq!(word_bounds(u, 10), (7, 21)); // "10.121.121.166" (dots join)
    assert_eq!(word_bounds(u, 21), (21, 22)); // ":"
    assert_eq!(word_bounds(u, 23), (22, 26)); // "3032"
  }

  #[test]
  fn locate_maps_pixels_to_line_and_col() {
    let b = book(&["0123456789"]); // the helper sets col = 20
    let font = 20.0;
    let adv = layout::char_advance(font); // 12.0 px
    // The block is `col`-wide (20*12 = 240) and centered in the 300px viewport,
    // so every line shares a 30px left margin — independent of its own length.
    let width = 300.0_f32;
    // x at the 3rd char center → col 3; y within first line.
    let x = 30.0 + 3.0 * adv as f32;
    assert_eq!(locate(&b, x, 5.0, 0.0, width, font, font as f32), (0, 3));
    // Clicking far right clamps to the line length.
    assert_eq!(locate(&b, 999.0, 5.0, 0.0, width, font, font as f32).1, 10);
  }
}
