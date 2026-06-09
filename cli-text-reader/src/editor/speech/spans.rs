use cli_pdf_to_text::PdfLineKind;

use super::WordSpan;

// Byte ranges of whitespace-separated words within a single line. UTF-8 safe:
// indices come from `char_indices`, so every boundary is a char boundary.
pub(crate) fn word_byte_ranges(line: &str) -> Vec<(usize, usize)> {
  let mut ranges = Vec::new();
  let mut start: Option<usize> = None;
  for (idx, ch) in line.char_indices() {
    if ch.is_whitespace() {
      if let Some(s) = start.take() {
        ranges.push((s, idx));
      }
    } else if start.is_none() {
      start = Some(idx);
    }
  }
  if let Some(s) = start {
    ranges.push((s, line.len()));
  }
  ranges
}

pub(crate) fn is_ansi_art_line(
  line_kinds: &[PdfLineKind],
  line_idx: usize,
) -> bool {
  line_kinds.get(line_idx) == Some(&PdfLineKind::AnsiArt)
}

// Build the narration word list from the on-screen lines. The absolute-offset
// accumulation MUST match `persistent_highlight_line_range`: visible text lines
// contribute `len + 1` (the implicit newline), even when narration skips their
// contents. AnsiArt lines are skipped entirely and contribute nothing.
pub(crate) fn build_word_spans(
  lines: &[String],
  line_kinds: &[PdfLineKind],
) -> Vec<WordSpan> {
  let mut spans = Vec::new();
  let mut abs = 0usize;
  let skip_narration = cli_justify::pdf_hybrid_narration_skip_mask(lines);
  for (line_idx, line) in lines.iter().enumerate() {
    if is_ansi_art_line(line_kinds, line_idx) {
      continue; // not in the coordinate space, not narrated
    }
    if !skip_narration.get(line_idx).copied().unwrap_or(false) {
      for (col_start, col_end) in word_byte_ranges(line) {
        spans.push(WordSpan {
          abs_start: abs + col_start,
          abs_end: abs + col_end,
          line: line_idx,
          col_start,
          col_end,
        });
      }
    }
    abs += line.len() + 1;
  }
  spans
}
