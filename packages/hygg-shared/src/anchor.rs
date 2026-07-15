//! Rendering-independent reading-position anchor shared by every hygg client
//! (CLI, PWA, GUI), so a position saved on one resumes on the *exact same
//! content* on any other — at any wrap width, with or without ASCII-image
//! rendering, and across client versions that render differently.
//!
//! # Why a character count
//!
//! A reader wraps the extracted text into rendered lines at its own column
//! width, so a *line index* is not portable. The one quantity that survives
//! wrapping is the **count of non-whitespace characters** before a position:
//! justification only inserts spaces, wrapping only turns an inter-word space
//! into a line break, and an over-long token that a narrow reader hard-splits
//! across two lines still carries the same characters. None of these change
//! how many non-whitespace characters precede a given piece of content, so the
//! count is width-independent. Whitespace (indentation, justification padding,
//! inter-column gaps) is deliberately ignored for the same reason.
//!
//! Image (ASCII-art) rows contribute nothing, so turning image rendering on or
//! off — which only adds or removes those rows — leaves every text anchor
//! unchanged.
//!
//! For PDFs the anchor is **page-local** (characters within the page, paired
//! with the 1-based page number) so it needs only the target page — always
//! preloaded on open — to resolve while later pages still stream in. Reflowable
//! formats have no pages, so it is a **global** offset from the document start.
//!
//! On the sync wire the value is carried by the historically named
//! `word_offset` field (it once counted whitespace-delimited words, which the
//! justifier's width-dependent hard-splitting made non-portable); it now holds
//! this non-whitespace character offset. Callers pair it with `page` to signal
//! page-local vs global, preserving the invariant *"the offset is page-local
//! iff a page is present."*

/// Non-whitespace characters on a single rendered line — the per-line unit the
/// anchor sums. Image rows are handled by the caller's `is_image` predicate
/// (they count zero), so this is only ever called on text rows.
#[inline]
pub fn line_units(line: &str) -> u64 {
  line.chars().filter(|c| !c.is_whitespace()).count() as u64
}

/// The anchor for `line`: the number of non-whitespace characters in
/// `lines[start..line]`, image rows excluded. `start` is the page's first line
/// for PDFs (page-local) or `0` for reflowable formats (global). `is_image(i)`
/// reports whether rendered line `i` is an image row.
///
/// Clamps `line` to the buffer, so an out-of-range line counts the whole range.
pub fn offset_of_line<F: Fn(usize) -> bool>(
  lines: &[String],
  is_image: F,
  start: usize,
  line: usize,
) -> u64 {
  let end = line.min(lines.len());
  let start = start.min(end);
  lines[start..end]
    .iter()
    .enumerate()
    .map(|(off, l)| if is_image(start + off) { 0 } else { line_units(l) })
    .sum()
}

/// The line in `lines[start..end]` that holds the character at anchor `target`
/// (counted from `start`), returned as an **absolute** line index. This is the
/// inverse of [`offset_of_line`]: the first line whose cumulative unit count
/// exceeds `target`. Clamps to the last line of the range when `target` is past
/// the end, so a resume never points outside the page/document.
pub fn line_for_offset<F: Fn(usize) -> bool>(
  lines: &[String],
  is_image: F,
  start: usize,
  end: usize,
  target: u64,
) -> usize {
  let end = end.min(lines.len());
  let start = start.min(end);
  let mut acc = 0u64;
  for (off, l) in lines[start..end].iter().enumerate() {
    let units = if is_image(start + off) { 0 } else { line_units(l) };
    if acc + units > target {
      return start + off;
    }
    acc += units;
  }
  end.saturating_sub(1).max(start)
}

/// Total non-whitespace characters in the whole document (image rows excluded)
/// — the denominator of the width-independent reading fraction.
pub fn total_units<F: Fn(usize) -> bool>(lines: &[String], is_image: F) -> u64 {
  offset_of_line(lines, is_image, 0, lines.len())
}

/// Width-independent reading fraction of `line`, in `0.0..=1.0`: the
/// non-whitespace characters before it over the document total. `0.0` for an
/// empty document.
///
/// This is the reading-percent metric every hygg client shares. A *line index*
/// over the line count is not portable — a wider reader wraps the same text
/// into fewer lines, so the same content sits at a different line-percent (the
/// "one client says 81%, another 88%" bug). The character fraction is the same
/// on every client at any width, exactly like the resume anchor it mirrors, so
/// the reader indicator, the library, and the server all agree.
pub fn fraction_of_line<F: Fn(usize) -> bool + Copy>(
  lines: &[String],
  is_image: F,
  line: usize,
) -> f64 {
  let total = total_units(lines, is_image);
  if total == 0 {
    return 0.0;
  }
  (offset_of_line(lines, is_image, 0, line) as f64 / total as f64)
    .clamp(0.0, 1.0)
}

/// The line at reading fraction `frac` (`0.0..=1.0`) — the inverse of
/// [`fraction_of_line`], for resuming a position that a differently-wrapped
/// reader synced as a percentage. Clamps `frac` into range.
pub fn line_for_fraction<F: Fn(usize) -> bool + Copy>(
  lines: &[String],
  is_image: F,
  frac: f64,
) -> usize {
  let total = total_units(lines, is_image);
  let target = (frac.clamp(0.0, 1.0) * total as f64).round() as u64;
  line_for_offset(lines, is_image, 0, lines.len(), target)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn lines(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
  }

  // An image row is line index 2 in these fixtures.
  fn is_image(i: usize) -> bool {
    i == 2
  }

  #[test]
  fn counts_non_whitespace_skipping_image_rows() {
    // "one two" -> 6 non-ws chars, "three" -> 5, image -> 0, "four" -> 4.
    let l = lines(&["one two", "three", "\u{2580}\u{2580}\u{2580}", "four"]);
    assert_eq!(offset_of_line(&l, is_image, 0, 4), 15);
    assert_eq!(offset_of_line(&l, is_image, 0, 2), 11);
    // The image row's own (block-glyph) characters must not be counted.
    assert_eq!(offset_of_line(&l, is_image, 2, 3), 0);
  }

  #[test]
  fn ignores_whitespace_so_padding_and_indent_do_not_shift_the_anchor() {
    // Same words, different wrapping/justification/indentation: identical
    // non-whitespace character totals, so the anchor is width-independent.
    let narrow = lines(&["hello", "world", "again"]);
    let wide = lines(&["hello   world", "    again"]);
    let none = |_: usize| false;
    assert_eq!(
      offset_of_line(&narrow, none, 0, narrow.len()),
      offset_of_line(&wide, none, 0, wide.len()),
    );
  }

  #[test]
  fn hard_split_token_keeps_the_same_total() {
    // A narrow reader chops one long token across two lines (no hyphen added);
    // the character total before/after is unchanged versus the whole token.
    let whole = lines(&["supercalifragilistic"]);
    let split = lines(&["supercalif", "ragilistic"]);
    let none = |_: usize| false;
    assert_eq!(
      offset_of_line(&whole, none, 0, 1),
      offset_of_line(&split, none, 0, 2),
    );
  }

  #[test]
  fn line_for_offset_is_the_inverse_and_returns_absolute_index() {
    let l = lines(&["one two", "three", "\u{2580}\u{2580}\u{2580}", "four"]);
    // Char 0 is on line 0; char 6 (the "t" of "three") is the 7th non-ws char,
    // so target 6 lands on line 1; the image row is skipped to reach "four".
    assert_eq!(line_for_offset(&l, is_image, 0, 4, 0), 0);
    assert_eq!(line_for_offset(&l, is_image, 0, 4, 6), 1);
    assert_eq!(line_for_offset(&l, is_image, 0, 4, 11), 3);
    // Past the end clamps to the last line of the range.
    assert_eq!(line_for_offset(&l, is_image, 0, 4, 9_999), 3);
  }

  #[test]
  fn fraction_is_width_independent() {
    // Same words wrapped two ways: identical reading fraction at the midpoint.
    // Two chars per token, 8 total; the first half is 4 chars either way.
    let narrow = lines(&["ab", "cd", "ef", "gh"]);
    let wide = lines(&["ab cd", "ef gh"]);
    let none = |_: usize| false;
    assert!((fraction_of_line(&narrow, none, 2) - 0.5).abs() < 1e-9);
    assert!((fraction_of_line(&wide, none, 1) - 0.5).abs() < 1e-9);
  }

  #[test]
  fn fraction_and_line_for_fraction_round_trip() {
    let l = lines(&["one two", "three", "four", "five six"]);
    let none = |_: usize| false;
    for line in 0..l.len() {
      let frac = fraction_of_line(&l, none, line);
      assert_eq!(line_for_fraction(&l, none, frac), line);
    }
  }

  #[test]
  fn empty_document_reads_zero() {
    let none = |_: usize| false;
    assert_eq!(fraction_of_line(&[], none, 0), 0.0);
  }

  #[test]
  fn page_local_round_trip_within_a_page() {
    // Page bounds [1, 4): resolving the offset of a line returns that line.
    let l = lines(&["cover", "alpha beta", "gamma", "delta"]);
    let none = |_: usize| false;
    for line in 1..4 {
      let off = offset_of_line(&l, none, 1, line);
      assert_eq!(line_for_offset(&l, none, 1, 4, off), line);
    }
  }
}
