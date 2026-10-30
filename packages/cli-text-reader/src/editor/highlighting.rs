// Line highlighting: the one place that decides how a line's text is styled.
//
// Every view (single pane and both split panes) and every output (the
// buffered frame and the immediate, non-terminal one) draws a highlighted line
// the same way:
//
//   1. `line_highlights` collects the line's highlight sources — visual
//      selection, search (or search preview) match, persistent highlights and
//      the spoken word — from the state that owns the line, and resolves them
//      into sorted, non-overlapping, char-boundary-safe byte ranges.
//   2. `write_highlighted_line` emits `center offset + segments + Clear` to any
//      writer.
//
// `render_line_highlights` glues the two together for the draw loops.
//
// Overlap policy: each byte takes the style of the highest-priority source
// covering it, SpokenWord > Selection > Search > Persistent. Lower-priority
// sources stay visible wherever nothing above them covers the line.

use crossterm::{
  cursor::MoveTo,
  queue,
  style::{Color, ResetColor, SetBackgroundColor, SetForegroundColor},
  terminal::{Clear, ClearType},
};
use std::io::{Result as IoResult, Write};

use super::char_boundary::{ByteCol, settle_span, slice, slice_from};
use super::core::Editor;

// Declared lowest priority first: the derived `Ord` is the overlap policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum HighlightKind {
  Persistent,
  Search,
  Selection,
  SpokenWord,
}

impl HighlightKind {
  fn colors(self) -> (Color, Color) {
    match self {
      HighlightKind::Persistent | HighlightKind::Search => {
        (Color::Yellow, Color::Black)
      }
      HighlightKind::Selection => (Color::DarkBlue, Color::White),
      HighlightKind::SpokenWord => (Color::Cyan, Color::Black),
    }
  }
}

// Byte range `start..end` of a line and how to style it.
pub(crate) type HighlightRange = (ByteCol, ByteCol, HighlightKind);

// Which view a line is drawn in, and so whose state its highlights come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LineView {
  // Single-pane view (normal and overlay): the active editor state, the
  // document in `self.lines`, and the narration's spoken word.
  Main,
  // One pane of the split view, showing `buffers[idx]`. The active buffer's
  // live state is in `editor_state`; an inactive one keeps its own. Only the
  // main buffer (0) carries persistent highlights; there is no narration.
  Pane(usize),
}

impl Editor {
  // Draw the cursor-line background bar; the line's text is drawn over it.
  pub fn highlight_current_line(
    &self,
    out: &mut impl Write,
    line_index: usize,
    term_width: u16,
  ) -> IoResult<bool> {
    // Suppress the highlight bar while the PDF is still being opened in
    // the background. cursor_y is already pre-positioned to the row the
    // streaming install will land on, so once pdf_pending clears the bar
    // appears in its final position without a jump.
    if self.pdf_pending.is_some() {
      return Ok(false);
    }
    if self.show_highlighter && line_index == self.cursor_y {
      self.debug_log(&format!(
        "Highlighting line {} with width {} (view_mode: {:?})",
        line_index, term_width, self.view_mode
      ));

      // Fill the entire width with the bar's background, then return to the
      // start of the row for the text
      queue!(
        out,
        MoveTo(0, line_index as u16),
        SetBackgroundColor(Color::Rgb { r: 40, g: 40, b: 40 })
      )?;
      write!(out, "{}", " ".repeat(term_width as usize))?;
      queue!(out, MoveTo(0, line_index as u16), ResetColor)?;

      Ok(true)
    } else {
      Ok(false)
    }
  }

  // The styled ranges of document line `line_idx` (text `line`) as drawn in
  // `view`. `None` when no highlight source touches the line, so the caller
  // draws it plainly. `Some` may hold no ranges: a selection or search match
  // that is on the line but empty (just after `v`) still claims it, which
  // keeps the line drawn — and the cursor bar cleared past its text — the way
  // a highlighted line is.
  pub(crate) fn line_highlights(
    &self,
    view: LineView,
    line_idx: usize,
    line: &str,
  ) -> Option<Vec<HighlightRange>> {
    let mut claimed = false;
    let mut ranges: Vec<HighlightRange> = Vec::new();
    let mut add = |range: (ByteCol, ByteCol), kind| {
      // Every range is settled into the line and onto char boundaries here,
      // once, so nothing downstream can slice inside a character (#99).
      let (start, end) = settle_span(line, range.0, range.1);
      if start < end {
        ranges.push((start, end, kind));
      }
    };

    if let Some(cols) = self.selection_columns(view, line_idx, line) {
      claimed = true;
      add(cols, HighlightKind::Selection);
    }
    if let Some(cols) = self.search_columns(view, line_idx) {
      claimed = true;
      add(cols, HighlightKind::Search);
    }
    for cols in self.persistent_columns(view, line_idx, line) {
      add(cols, HighlightKind::Persistent);
    }
    if view == LineView::Main
      && let Some(cols) = self.spoken_word_columns(line_idx, line)
    {
      add(cols, HighlightKind::SpokenWord);
    }

    if ranges.is_empty() && !claimed {
      return None;
    }
    Some(resolve_overlaps(&ranges))
  }

  // Draw document line `line_idx` with its highlights, if it has any. Returns
  // whether it drew the line; if not, the caller draws it plainly.
  pub(crate) fn render_line_highlights(
    &self,
    out: &mut impl Write,
    view: LineView,
    line_idx: usize,
    line: &str,
    center_offset_string: &str,
  ) -> IoResult<bool> {
    match self.line_highlights(view, line_idx, line) {
      Some(ranges) => {
        write_highlighted_line(out, line, &ranges, center_offset_string)?;
        Ok(true)
      }
      None => Ok(false),
    }
  }

  // The current search match on this line, if any. Search stores byte offsets
  // it found in a lowercased copy of the line, and lowercasing can change a
  // string's byte layout — 'İ' is two bytes, its lowercase form is three — so
  // the offsets are not guaranteed to land on this line's character boundaries,
  // or even inside it. `line_highlights` settles them like every other range.
  fn search_columns(
    &self,
    view: LineView,
    line_idx: usize,
  ) -> Option<(ByteCol, ByteCol)> {
    let found = match view {
      LineView::Pane(idx) if idx != self.active_buffer => {
        self.buffers.get(idx)?.current_match
      }
      _ if self.editor_state.search_preview_active => {
        self.editor_state.search_preview_match
      }
      _ => self.editor_state.current_match,
    };
    found.filter(|m| m.line == line_idx).map(|m| (m.start, m.end))
  }
}

// Flatten possibly-overlapping ranges into sorted, disjoint runs: each byte
// takes the highest-priority kind covering it, and adjacent runs of the same
// kind merge. The runs are cut only where an input range starts or ends, so
// they stay on the inputs' char boundaries.
fn resolve_overlaps(ranges: &[HighlightRange]) -> Vec<HighlightRange> {
  let mut cuts: Vec<ByteCol> =
    ranges.iter().flat_map(|&(start, end, _)| [start, end]).collect();
  cuts.sort_unstable();
  cuts.dedup();

  let mut runs: Vec<HighlightRange> = Vec::new();
  for pair in cuts.windows(2) {
    let (from, to) = (pair[0], pair[1]);
    // Coverage is constant between two consecutive cuts.
    let Some(kind) = ranges
      .iter()
      .filter(|&&(start, end, _)| start <= from && to <= end)
      .map(|&(_, _, kind)| kind)
      .max()
    else {
      continue;
    };
    match runs.last_mut() {
      Some(run) if run.1 == from && run.2 == kind => run.1 = to,
      _ => runs.push((from, to, kind)),
    }
  }
  runs
}

// Write `center offset + line` with `ranges` styled, then clear the rest of the
// row. `ranges` should be sorted and disjoint (as `line_highlights` returns
// them); anything else is clamped rather than sliced blindly.
pub(crate) fn write_highlighted_line(
  out: &mut impl Write,
  line: &str,
  ranges: &[HighlightRange],
  center_offset_string: &str,
) -> IoResult<()> {
  write!(out, "{center_offset_string}")?;
  let mut pos = ByteCol::ZERO;
  for &(start, end, kind) in ranges {
    let (start, end) = settle_span(line, start.max(pos), end.max(pos));
    let (background, foreground) = kind.colors();
    write!(out, "{}", slice(line, pos, start))?;
    queue!(
      out,
      SetBackgroundColor(background),
      SetForegroundColor(foreground)
    )?;
    write!(out, "{}", slice(line, start, end))?;
    queue!(out, ResetColor)?;
    pos = end;
  }
  write!(out, "{}", slice_from(line, pos))?;
  queue!(out, Clear(ClearType::UntilNewLine))
}

#[cfg(test)]
mod tests;
