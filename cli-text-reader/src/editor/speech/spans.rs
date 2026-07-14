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

// Split a single whitespace word at a sentence mark wedged between a lowercase
// and an uppercase letter ("end.Start" -> "end." + "Start"). PDF text
// extraction routinely glues a sentence's end onto the next sentence's start
// (no space), which leaves the mark *inside* a word: the synthesizer then reads
// it as one run with no pause, and mispronounces it. Splitting here gives each
// half its own narration word (so the mark becomes a pause token) and its own
// highlight span, without touching the displayed line. Marks after a digit
// ("3.14"), after an uppercase letter ("U.S"), or before a lowercase letter
// ("e.g.") are left alone, so decimals and common abbreviations stay intact.
// Returns byte sub-ranges *relative to `word`*; a word with no such glue yields
// a single full-width range.
fn split_glued_sentence(word: &str) -> Vec<(usize, usize)> {
  let chars: Vec<(usize, char)> = word.char_indices().collect();
  let mut ranges = Vec::new();
  let mut start = 0usize;
  for w in 1..chars.len() {
    let (i, c) = chars[w];
    let after_lower = chars[w - 1].1.is_lowercase();
    let before_upper = chars.get(w + 1).is_some_and(|(_, n)| n.is_uppercase());
    if matches!(c, '.' | '!' | '?') && after_lower && before_upper {
      let split_at = i + c.len_utf8(); // keep the mark with the left half
      ranges.push((start, split_at));
      start = split_at;
    }
  }
  ranges.push((start, word.len()));
  ranges
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
        // Split glued sentences ("end.Start") so each half narrates and
        // highlights on its own; a normal word yields one full-width range.
        for (rel_start, rel_end) in
          split_glued_sentence(&line[col_start..col_end])
        {
          let (ws, we) = (col_start + rel_start, col_start + rel_end);
          spans.push(WordSpan {
            abs_start: abs + ws,
            abs_end: abs + we,
            line: line_idx,
            col_start: ws,
            col_end: we,
          });
        }
      }
    }
    abs += line.len() + 1;
  }
  spans
}
