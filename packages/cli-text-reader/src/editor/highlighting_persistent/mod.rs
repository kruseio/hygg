use cli_pdf_to_text::PdfLineKind;

use super::char_boundary::ByteCol;
use super::core::Editor;
use super::highlighting::LineView;

impl Editor {
  // Absolute `start..end` of a document line in the persistent-highlight
  // coordinate space: text lines laid end to end with an implicit newline each,
  // AnsiArt rows skipped. `None` for an AnsiArt row, which has no text.
  pub(crate) fn persistent_highlight_line_range(
    line_index: usize,
    lines: &[String],
    line_kinds: &[PdfLineKind],
  ) -> Option<(usize, usize)> {
    if line_kinds.get(line_index) == Some(&PdfLineKind::AnsiArt) {
      return None;
    }

    let mut abs_line_start = 0;
    for i in 0..line_index {
      if i < lines.len() && line_kinds.get(i) != Some(&PdfLineKind::AnsiArt) {
        abs_line_start += lines[i].len() + 1;
      }
    }

    let abs_line_end = if line_index < lines.len() {
      abs_line_start + lines[line_index].len()
    } else {
      abs_line_start
    };

    Some((abs_line_start, abs_line_end))
  }

  // Byte columns of the persistent highlights on document line `line_idx`
  // (text `line`) as drawn in `view`. Raw: `line_highlights` settles them onto
  // the line, which matters when the offsets predate a reflow.
  pub(crate) fn persistent_columns(
    &self,
    view: LineView,
    line_idx: usize,
    line: &str,
  ) -> Vec<(ByteCol, ByteCol)> {
    // Persistent highlights belong to the main document: in the single-pane
    // view they are laid over whatever `self.lines` shows, in the split view
    // only over the main buffer's pane.
    let (lines, line_kinds) = match view {
      LineView::Main => (&self.lines, &self.line_kinds),
      LineView::Pane(0) => match self.buffers.first() {
        Some(buffer) => (&buffer.lines, &buffer.line_kinds),
        None => return Vec::new(),
      },
      LineView::Pane(_) => return Vec::new(),
    };
    let Some((abs_start, abs_end)) =
      Self::persistent_highlight_line_range(line_idx, lines, line_kinds)
    else {
      return Vec::new();
    };

    self
      .highlights
      .get_highlights_for_range(abs_start, abs_end)
      .into_iter()
      .map(|h| {
        // Absolute offsets from disk or another device, so floored onto the
        // line rather than trusted.
        let start = h.start.saturating_sub(abs_start);
        let end = if h.end >= abs_end { line.len() } else { h.end - abs_start };
        (ByteCol::floor(line, start), ByteCol::floor(line, end))
      })
      .filter(|&(start, end)| end > start && start < ByteCol::end(line))
      .collect()
  }

  // Does screen row `line_index` of the single-pane view carry a persistent
  // highlight?
  #[cfg(test)]
  pub fn has_persistent_highlights_on_line(&self, line_index: usize) -> bool {
    let line_idx = self.offset + line_index;
    let line = self.lines.get(line_idx).map_or("", String::as_str);
    !self.persistent_columns(LineView::Main, line_idx, line).is_empty()
  }
}
