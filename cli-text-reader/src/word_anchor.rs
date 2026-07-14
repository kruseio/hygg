//! Rendering-independent reading-position anchor for exact cross-device resume.
//!
//! The anchor is the count of **non-whitespace characters** before a rendered
//! line (image rows excluded), page-local for PDFs and global otherwise. It is
//! width-independent because wrapping/justification only move whitespace around
//! and an over-long token hard-split across lines keeps its characters, so two
//! readers at different column widths land on the same content. The shared math
//! lives in [`hygg_shared::anchor`] so every hygg client agrees exactly; the
//! helpers here just adapt this reader's `PdfLineKind` rows to it.
//!
//! (The functions and the synced `word_offset` field keep their historical
//! "word" names — the anchor once counted whitespace-delimited words, which the
//! justifier's width-dependent hard-splitting made non-portable.)

use cli_pdf_to_text::PdfLineKind;

/// True when rendered line `i` is an image (ASCII-art) row, which contributes
/// no characters to the anchor. `kinds` may be shorter than `lines` (older or
/// non-PDF buffers); a missing entry is treated as text.
fn is_image_row(kinds: &[PdfLineKind], i: usize) -> bool {
  matches!(kinds.get(i), Some(PdfLineKind::AnsiArt))
}

/// The anchor for `lines[start..end)`: non-whitespace characters in that range,
/// image rows excluded. `start` is the page's first line (page-local) or 0
/// (global).
pub fn words_in_range(
  lines: &[String],
  kinds: &[PdfLineKind],
  start: usize,
  end: usize,
) -> usize {
  hygg_shared::anchor::offset_of_line(
    lines,
    |i| is_image_row(kinds, i),
    start,
    end,
  ) as usize
}

/// Width-independent reading fraction (`0.0..=1.0`) of `line`: global
/// non-whitespace characters before it over the document total. The metric the
/// reader indicator and synced `percentage` both use, so the same content reads
/// the same percent on every client at any wrap width.
pub fn fraction_of_line(
  lines: &[String],
  kinds: &[PdfLineKind],
  line: usize,
) -> f64 {
  hygg_shared::anchor::fraction_of_line(lines, |i| is_image_row(kinds, i), line)
}

/// The line at reading fraction `frac` — the inverse of [`fraction_of_line`],
/// for resuming a position a differently-wrapped reader synced as a percentage.
pub fn line_for_fraction(
  lines: &[String],
  kinds: &[PdfLineKind],
  frac: f64,
) -> usize {
  hygg_shared::anchor::line_for_fraction(
    lines,
    |i| is_image_row(kinds, i),
    frac,
  )
}

/// The line in `lines[start..end)` holding anchor `target_word` (counted from
/// `start`), returned as an offset from `start`. Clamps to the last line of the
/// range when the anchor is past the end.
pub fn line_for_word_in_range(
  lines: &[String],
  kinds: &[PdfLineKind],
  start: usize,
  end: usize,
  target_word: usize,
) -> usize {
  hygg_shared::anchor::line_for_offset(
    lines,
    |i| is_image_row(kinds, i),
    start,
    end,
    target_word as u64,
  )
  .saturating_sub(start)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn lines(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
  }

  #[test]
  fn counts_characters_skipping_image_rows() {
    let l = lines(&["one two", "three", "\u{2580}\u{2580}\u{2580}", "four"]);
    let k = vec![
      PdfLineKind::Text,
      PdfLineKind::Text,
      PdfLineKind::AnsiArt,
      PdfLineKind::Text,
    ];
    // 6 + 5 + 0 + 4 non-whitespace chars.
    assert_eq!(words_in_range(&l, &k, 0, 4), 15);
    assert_eq!(words_in_range(&l, &k, 0, 2), 11);
  }

  #[test]
  fn maps_anchor_back_to_its_line_offset_from_start() {
    let l = lines(&["one two", "three", "\u{2580}\u{2580}\u{2580}", "four"]);
    let k = vec![
      PdfLineKind::Text,
      PdfLineKind::Text,
      PdfLineKind::AnsiArt,
      PdfLineKind::Text,
    ];
    assert_eq!(line_for_word_in_range(&l, &k, 0, 4, 0), 0);
    assert_eq!(line_for_word_in_range(&l, &k, 0, 4, 6), 1);
    assert_eq!(line_for_word_in_range(&l, &k, 0, 4, 11), 3);
    // Offset is relative to `start` (used as line-in-page for PDFs).
    assert_eq!(line_for_word_in_range(&l, &k, 1, 4, 5), 2);
  }
}
