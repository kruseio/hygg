use crate::text_utils::char_len;

use crate::pdf_hybrid::structure::looks_like_table_or_figure_caption;

// How many wrapped lines of one list item / caption we'll scan back through
// to recognise a sibling. Real captions wrap to 2–3 lines; bumping a bit for
// margin without making the scan unbounded.
const MAX_SIBLING_LOOKBACK_LINES: usize = 12;

/// True if `line` is the first line of a list item that shares `indent` and
/// `marker` with a new item we're about to start. Bullet markers must match
/// exactly. Numeric markers (`1.`, `12)`) match by shape — same indent and
/// the same punctuation style — since each item carries a different counter.
fn line_starts_sibling_list_item(
  line: &str,
  indent: &str,
  marker: &str,
) -> bool {
  if line.starts_with(&format!("{indent}{marker}")) {
    return true;
  }
  let Some(rest) = line.strip_prefix(indent) else {
    return false;
  };
  let marker_punct = marker.trim_end().chars().last();
  let Some(marker_punct) = marker_punct else {
    return false;
  };
  if marker_punct != '.' && marker_punct != ')' {
    return false;
  }
  let digit_count = rest.chars().take_while(|ch| ch.is_ascii_digit()).count();
  if digit_count == 0 {
    return false;
  }
  let mut after_digits = rest.chars().skip(digit_count);
  let Some(delim) = after_digits.next() else {
    return false;
  };
  if delim != marker_punct {
    return false;
  }
  matches!(after_digits.next(), Some(' '))
}

/// When a bulleted / numbered list spans a PDF page break the extractor
/// emits one or more blank lines between sibling items. The list reads as
/// one continuous block in the source PDF, so the blanks split it
/// artificially. If the most recently emitted block is a sibling list item,
/// drop the trailing blanks.
pub(crate) fn drop_trailing_blanks_after_sibling_list(
  out: &mut Vec<String>,
  indent: &str,
  marker: &str,
) {
  let mut blanks_start = out.len();
  while blanks_start > 0 && out[blanks_start - 1].is_empty() {
    blanks_start -= 1;
  }
  if blanks_start == out.len() || blanks_start == 0 {
    return;
  }
  let continuation_indent_width = char_len(indent) + char_len(marker);
  let scan_floor = blanks_start.saturating_sub(MAX_SIBLING_LOOKBACK_LINES);
  for idx in (scan_floor..blanks_start).rev() {
    let line = &out[idx];
    if line.is_empty() {
      return;
    }
    if line_starts_sibling_list_item(line, indent, marker) {
      out.truncate(blanks_start);
      return;
    }
    // Continuation lines of the prior list item carry at least the
    // continuation indent. A shorter indent means we've left the prior
    // item without finding its marker line — different block.
    let leading_ws = line.chars().take_while(|ch| *ch == ' ').count();
    if leading_ws < continuation_indent_width {
      return;
    }
  }
}

/// Same idea as `drop_trailing_blanks_after_sibling_list`, but for the
/// front-matter caption lists (`Plate 1 …`, `Figure 3.4 …`, `Table 2 …`).
/// These are emitted as standalone paragraphs, so a page-break blank lands
/// directly between two caption entries and stretches the list out.
///
/// Returns `true` when the most recent flushed content is part of a caption
/// (with or without trailing blanks between it and the next caption). Drops
/// the trailing blanks as a side effect so the captions read as one block.
/// Also reports `true` when the prior caption was emitted via the
/// preserved-layout path (wide internal gaps); without this the paragraph
/// handler treats the new caption as a "prose → caption" transition and
/// inserts a spurious separator blank.
pub(crate) fn out_ends_in_caption_context(out: &mut Vec<String>) -> bool {
  let mut blanks_start = out.len();
  while blanks_start > 0 && out[blanks_start - 1].is_empty() {
    blanks_start -= 1;
  }
  if blanks_start == 0 {
    return false;
  }
  let had_blanks = blanks_start < out.len();
  let scan_floor = blanks_start.saturating_sub(MAX_SIBLING_LOOKBACK_LINES);
  for idx in (scan_floor..blanks_start).rev() {
    let line = &out[idx];
    if line.is_empty() {
      return false;
    }
    if looks_like_table_or_figure_caption(line.trim()) {
      if had_blanks {
        out.truncate(blanks_start);
      }
      return true;
    }
  }
  false
}
