use cli_pdf_to_text::PdfLineKind;

use super::super::core::Editor;

impl Editor {
  // Check if a line has persistent highlights
  pub fn has_persistent_highlights_on_line(&self, line_index: usize) -> bool {
    self.has_persistent_highlights_on_line_with_offset(line_index, self.offset)
  }

  // Check if a line has persistent highlights with custom offset
  pub fn has_persistent_highlights_on_line_with_offset(
    &self,
    line_index: usize,
    offset: usize,
  ) -> bool {
    self.has_persistent_highlights_on_line_with_offset_and_lines(
      line_index,
      offset,
      &self.lines,
    )
  }

  // Check if a line has persistent highlights with custom offset and lines
  pub fn has_persistent_highlights_on_line_with_offset_and_lines(
    &self,
    line_index: usize,
    offset: usize,
    lines: &[String],
  ) -> bool {
    self.has_persistent_highlights_on_line_with_offset_lines_and_kinds(
      line_index,
      offset,
      lines,
      &self.line_kinds,
    )
  }

  pub(crate) fn has_persistent_highlights_on_line_with_offset_lines_and_kinds(
    &self,
    line_index: usize,
    offset: usize,
    lines: &[String],
    line_kinds: &[PdfLineKind],
  ) -> bool {
    let current_line_idx = offset + line_index;

    let Some((abs_line_start, abs_line_end)) =
      Self::persistent_highlight_line_range(
        current_line_idx,
        lines,
        line_kinds,
      )
    else {
      return false;
    };

    // Check if any highlights overlap with this line
    let highlights_in_range =
      self.highlights.get_highlights_for_range(abs_line_start, abs_line_end);

    !highlights_in_range.is_empty()
  }
}
