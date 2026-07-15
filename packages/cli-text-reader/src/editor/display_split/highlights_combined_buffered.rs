use cli_pdf_to_text::PdfLineKind;
use crossterm::{
  QueueableCommand,
  style::{Color, ResetColor, SetBackgroundColor, SetForegroundColor},
  terminal::{Clear, ClearType},
};
use std::io::{Result as IoResult, Write};

use super::super::core::Editor;
use super::super::highlighting_persistent::HighlightType;

impl Editor {
  // Buffered version of render_pane_combined_highlights
  #[allow(clippy::too_many_arguments)]
  pub(crate) fn render_pane_combined_highlights_buffered(
    &self,
    buffer: &mut Vec<u8>,
    buffer_idx: usize,
    viewport_line_idx: usize,
    line: &str,
    center_offset_string: &str,
    offset: usize,
    buffer_lines: &[String],
    buffer_line_kinds: &[PdfLineKind],
  ) -> IoResult<bool> {
    // Only main buffer has persistent highlights
    if buffer_idx != 0 {
      return self.render_pane_selection_buffered(
        buffer,
        buffer_idx,
        viewport_line_idx,
        line,
        center_offset_string,
      );
    }

    let current_line_idx = offset + viewport_line_idx;

    // Get all highlight ranges for this line
    let mut ranges: Vec<(usize, usize, HighlightType)> = Vec::new();

    // Add visual selection range if present
    if let (Some(start), Some(end)) =
      (self.editor_state.selection_start, self.editor_state.selection_end)
    {
      let is_line_mode =
        self.editor_state.mode == super::super::core::EditorMode::VisualLine;

      if is_line_mode
        && current_line_idx >= start.0.min(end.0)
        && current_line_idx <= start.0.max(end.0)
      {
        ranges.push((0, line.len(), HighlightType::Selection));
      } else if !is_line_mode {
        // Handle character mode selection
        if start.0 == end.0 && current_line_idx == start.0 {
          let start_col = start.1.min(end.1);
          let end_col = start.1.max(end.1).min(line.len());
          if start_col < end_col {
            ranges.push((start_col, end_col, HighlightType::Selection));
          }
        } else if current_line_idx >= start.0.min(end.0)
          && current_line_idx <= start.0.max(end.0)
        {
          // Multi-line selection logic
          if current_line_idx == start.0.min(end.0) {
            let col = if start.0 < end.0 { start.1 } else { end.1 };
            ranges.push((col, line.len(), HighlightType::Selection));
          } else if current_line_idx == start.0.max(end.0) {
            let col = if start.0 > end.0 { start.1 } else { end.1 };
            ranges.push((0, col.min(line.len()), HighlightType::Selection));
          } else {
            ranges.push((0, line.len(), HighlightType::Selection));
          }
        }
      }
    }

    // Add persistent highlight ranges
    let Some((abs_line_start, abs_line_end)) =
      Self::persistent_highlight_line_range(
        current_line_idx,
        buffer_lines,
        buffer_line_kinds,
      )
    else {
      return Ok(false);
    };

    let line_highlights =
      self.highlights.get_highlights_for_range(abs_line_start, abs_line_end);
    for highlight in line_highlights {
      let start = if highlight.start <= abs_line_start {
        0
      } else {
        highlight.start - abs_line_start
      };
      let end = if highlight.end >= abs_line_end {
        line.len()
      } else {
        highlight.end - abs_line_start
      };

      if end > start && start < line.len() {
        ranges.push((
          start.min(line.len()),
          end.min(line.len()),
          HighlightType::Persistent,
        ));
      }
    }

    if ranges.is_empty() {
      return Ok(false);
    }

    // Sort ranges by start position
    ranges.sort_by_key(|r| r.0);

    // Render the line with all highlights
    write!(buffer, "{center_offset_string}")?;
    let mut last_end = 0;

    for (start, end, highlight_type) in ranges {
      // Print unhighlighted text before this highlight
      if start > last_end {
        write!(buffer, "{}", &line[last_end..start])?;
      }

      // Print highlighted text with appropriate style
      match highlight_type {
        HighlightType::Selection => {
          buffer.queue(SetBackgroundColor(Color::DarkBlue))?;
          buffer.queue(SetForegroundColor(Color::White))?;
        }
        HighlightType::Persistent => {
          buffer.queue(SetBackgroundColor(Color::Yellow))?;
          buffer.queue(SetForegroundColor(Color::Black))?;
        }
      }

      // Handle overlapping ranges - use the max end
      let actual_end = if last_end > start { last_end.max(end) } else { end };
      let actual_start = last_end.max(start);

      if actual_start < actual_end && actual_start < line.len() {
        write!(buffer, "{}", &line[actual_start..actual_end.min(line.len())])?;
      }

      buffer.queue(ResetColor)?;
      last_end = actual_end;
    }

    // Print remaining unhighlighted text
    if last_end < line.len() {
      write!(buffer, "{}", &line[last_end..])?;
    }

    // Clear to end of line to match normal view rendering
    buffer.queue(Clear(ClearType::UntilNewLine))?;

    Ok(true)
  }
}
