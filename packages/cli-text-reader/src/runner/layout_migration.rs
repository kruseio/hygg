//! One-time move of a document's saved state onto its new layout.
//!
//! hygg now decodes text input: a document that used to open with a UTF-8 BOM
//! (kept as a stray U+FEFF) or with CRLF line endings (which hid every
//! paragraph break from the justifier) is now shown without the BOM and with
//! its paragraphs. Its identity is deliberately unchanged — hygg still passes
//! the original text as `raw_content`, so the `document_hash` its progress,
//! highlights, bookmarks and notes are filed under, and the sync `book_id`, are
//! the same — but the positions inside that state refer to the old layout,
//! `cli_justify::justify(raw_content, col)`: highlights are byte offsets into
//! the joined lines (3 bytes off after a stripped BOM), bookmarks are (line,
//! byte column), notes and progress are line numbers.
//!
//! On the first open of such a document, every saved position is moved to the
//! same text in the new layout, and a per-document marker records that its
//! state now refers to the new layout, so this never runs twice. Documents
//! whose layout did not change (anything without a leading BOM or a CR) are
//! never touched.
//!
//! Positions are matched through the document's non-whitespace characters
//! (U+FEFF excluded), which both layouts share in the same order: a highlight
//! on "brown fox" lands on "brown fox" again however the lines rewrapped.

use crate::bookmarks::{load_bookmarks, save_bookmarks};
use crate::highlights::{HighlightData, load_highlights, save_highlights};
use crate::notes::{load_notes, save_notes};
use crate::progress::{
  Progress, generate_hash, load_progress, save_progress_full,
};
use std::collections::HashMap;

/// Move saved state for the document identified by `raw_content` from its old
/// layout onto `lines` (as passed to the reader, before sanitising).
pub(crate) fn migrate_saved_state(
  raw_content: &str,
  lines: &[String],
  col: usize,
) {
  if !layout_may_have_changed(raw_content) {
    return;
  }
  let hash = generate_hash(&raw_content);
  let Ok(marker) = crate::utils::get_hygg_subdir_file(
    "layout-migrations",
    &format!("{hash}.decoded-text-v1"),
  ) else {
    return;
  };
  if marker.exists() {
    return;
  }
  let old_lines = cli_justify::justify(raw_content, col);
  if old_lines != lines
    && let Some(map) = LayoutMap::new(&old_lines, lines)
  {
    migrate_files(hash, &map);
    crate::debug::debug_log(
      "layout_migration",
      "moved saved state to new layout",
    );
  }
  if let Err(e) = std::fs::write(&marker, b"") {
    crate::debug::debug_log_error("layout_migration", &format!("marker: {e}"));
  }
}

/// Only text that starts with a BOM or contains a CR is laid out differently
/// now; everything else (in particular BOM-less LF UTF-8) is byte-identical.
fn layout_may_have_changed(raw_content: &str) -> bool {
  raw_content.starts_with('\u{FEFF}') || raw_content.contains('\r')
}

fn migrate_files(hash: u64, map: &LayoutMap) {
  if let Ok(mut data) = load_highlights(&hash.to_string())
    && !data.highlights.is_empty()
  {
    remap_highlights(map, &mut data);
    let _ = save_highlights(&data);
  }
  if let Ok(data) = load_bookmarks(hash)
    && !data.marks.is_empty()
  {
    let _ = save_bookmarks(hash, &remap_marks(map, &data.marks));
  }
  if let Ok(mut data) = load_notes(hash)
    && data.notes.iter().any(|n| n.line.is_some())
  {
    for note in &mut data.notes {
      note.line = note.line.map(|line| map.line(line));
    }
    let _ = save_notes(hash, &data);
  }
  if let Ok(progress) = load_progress(hash)
    && progress.page.is_none()
  {
    let p = remap_progress(map, &progress);
    let _ = save_progress_full(
      hash,
      p.updated_at,
      p.offset,
      p.total_lines,
      p.percentage,
      p.viewport_offset,
      p.cursor_y,
      None,
      None,
      p.word_offset,
      p.reading_time_seconds,
    );
  }
}

/// Highlights are byte ranges into the joined lines. A range that covered
/// nothing but the stripped BOM disappears.
pub(super) fn remap_highlights(map: &LayoutMap, data: &mut HighlightData) {
  for h in &mut data.highlights {
    h.start = map.offset(h.start, false);
    h.end = map.offset(h.end, true);
  }
  data.highlights.retain(|h| h.end > h.start);
  data.highlights.sort_by_key(|h| h.start);
  data.highlights.dedup_by(|a, b| a.start == b.start && a.end == b.end);
}

pub(super) fn remap_marks(
  map: &LayoutMap,
  marks: &HashMap<char, (usize, usize)>,
) -> HashMap<char, (usize, usize)> {
  marks.iter().map(|(&mark, &pos)| (mark, map.position(pos))).collect()
}

/// Progress is a line (`offset`), an exact viewport (`viewport_offset` +
/// `cursor_y`), and the width-independent character anchor (`word_offset`),
/// which counts the U+FEFF the old layout had.
pub(super) fn remap_progress(map: &LayoutMap, p: &Progress) -> Progress {
  let (viewport_offset, cursor_y) = match (p.viewport_offset, p.cursor_y) {
    (Some(top), Some(row)) => {
      let line = map.line(top + row);
      let top = line.saturating_sub(row);
      (Some(top), Some(line - top))
    }
    other => other,
  };
  let word_offset = p.word_offset.map(|word| {
    let old_line = crate::word_anchor::line_for_word_in_range(
      &map.old_lines,
      &[],
      0,
      map.old_lines.len(),
      word,
    );
    let new_line = map.line(old_line);
    crate::word_anchor::words_in_range(&map.new_lines, &[], 0, new_line)
  });
  Progress {
    document_hash: p.document_hash,
    updated_at: p.updated_at,
    offset: map.line(p.offset),
    total_lines: map.new_lines.len(),
    percentage: p.percentage,
    viewport_offset,
    cursor_y,
    page: None,
    line_in_page: None,
    word_offset,
    reading_time_seconds: p.reading_time_seconds,
  }
}

/// Maps positions in the old layout to the same text in the new one.
pub(super) struct LayoutMap {
  old_lines: Vec<String>,
  new_lines: Vec<String>,
  old_starts: Vec<usize>,
  new_starts: Vec<usize>,
  new_len: usize,
  /// Byte offsets of the key characters in the joined old text.
  old_keys: Vec<usize>,
  /// Byte offsets and lengths of the same characters in the new text.
  new_keys: Vec<(usize, usize)>,
}

impl LayoutMap {
  /// `None` when the two layouts do not hold the same characters, in which
  /// case nothing can be mapped safely and the saved state is left alone.
  pub(super) fn new(
    old_lines: &[String],
    new_lines: &[String],
  ) -> Option<Self> {
    // Mirror the reader's constructor, which strips control characters from
    // every line before any offset is taken against it.
    let old_lines: Vec<String> =
      old_lines.iter().map(|l| sanitize(l)).collect();
    let new_lines: Vec<String> =
      new_lines.iter().map(|l| sanitize(l)).collect();
    let (old_starts, old_text) = join(&old_lines);
    let (new_starts, new_text) = join(&new_lines);
    let keys = |text: &str| -> Vec<(usize, char)> {
      text
        .char_indices()
        .filter(|&(_, c)| !c.is_whitespace() && c != '\u{FEFF}')
        .collect()
    };
    let (old_keys, new_keys) = (keys(&old_text), keys(&new_text));
    if !old_keys.iter().map(|k| k.1).eq(new_keys.iter().map(|k| k.1)) {
      return None;
    }
    Some(Self {
      old_lines,
      new_lines,
      old_starts,
      new_starts,
      new_len: new_text.len(),
      old_keys: old_keys.iter().map(|k| k.0).collect(),
      new_keys: new_keys.iter().map(|&(at, c)| (at, c.len_utf8())).collect(),
    })
  }

  /// Map a byte offset. A range start moves to the next key character; a
  /// range end (exclusive) to just after the previous one.
  pub(super) fn offset(&self, old: usize, end: bool) -> usize {
    let before = self.old_keys.partition_point(|&at| at < old);
    if end {
      before
        .checked_sub(1)
        .map_or(0, |k| self.new_keys[k].0 + self.new_keys[k].1)
    } else {
      self.new_keys.get(before).map_or(self.new_len, |k| k.0)
    }
  }

  /// Map a line: the new line holding the first text at or after the old
  /// line's start.
  pub(super) fn line(&self, old_line: usize) -> usize {
    let Some(&start) = self.old_starts.get(old_line) else {
      return self.new_lines.len().saturating_sub(1);
    };
    self.line_of(self.offset(start, false))
  }

  /// Map a bookmark's (line, byte column).
  pub(super) fn position(&self, (line, col): (usize, usize)) -> (usize, usize) {
    let Some(text) = self.old_lines.get(line) else {
      return (self.line(line), 0);
    };
    let at =
      self.offset(self.old_starts[line] + floor_boundary(text, col), false);
    let new_line = self.line_of(at);
    let new_col = at.saturating_sub(self.new_starts[new_line]);
    (new_line, floor_boundary(&self.new_lines[new_line], new_col))
  }

  fn line_of(&self, at: usize) -> usize {
    self.new_starts.partition_point(|&start| start <= at).saturating_sub(1)
  }
}

/// The joined text the reader takes highlight offsets against, and where each
/// line starts in it.
fn join(lines: &[String]) -> (Vec<usize>, String) {
  let mut starts = Vec::with_capacity(lines.len());
  let mut text = String::new();
  for line in lines {
    starts.push(text.len());
    text.push_str(line);
    text.push('\n');
  }
  (starts, text)
}

fn sanitize(line: &str) -> String {
  line.chars().filter(|&c| !c.is_control() || c == '\t').collect()
}

fn floor_boundary(text: &str, col: usize) -> usize {
  let mut col = col.min(text.len());
  while !text.is_char_boundary(col) {
    col -= 1;
  }
  col
}

#[cfg(test)]
mod tests;
