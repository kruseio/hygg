use cli_pdf_to_text::PdfLineKind;
use crossterm::{
  execute,
  style::{Color, ResetColor, SetBackgroundColor, SetForegroundColor},
  terminal::{Clear, ClearType},
};
use std::io::{self, Result as IoResult, Write};

use super::super::core::Editor;

impl Editor {
  // Render persistent highlights for a pane line
  #[allow(clippy::too_many_arguments)]
  pub(crate) fn render_pane_persistent_highlights(
    &self,
    stdout: &mut io::Stdout,
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
      return Ok(false);
    }

    // Get the actual line index in the main buffer
    let current_line_idx = offset + viewport_line_idx;

    let Some((abs_line_start, abs_line_end)) =
      Self::persistent_highlight_line_range(
        current_line_idx,
        buffer_lines,
        buffer_line_kinds,
      )
    else {
      return Ok(false);
    };

    // Get highlights that overlap with this line
    let line_highlights =
      self.highlights.get_highlights_for_range(abs_line_start, abs_line_end);

    if line_highlights.is_empty() {
      return Ok(false);
    }

    // Convert highlights to line-relative positions and merge overlapping
    // ranges
    let mut ranges: Vec<(usize, usize)> = Vec::new();
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
        ranges.push((start.min(line.len()), end.min(line.len())));
      }
    }

    if ranges.is_empty() {
      return Ok(false);
    }

    // Sort and merge overlapping ranges
    ranges.sort_by_key(|r| r.0);
    let mut merged_ranges: Vec<(usize, usize)> = Vec::new();
    for range in ranges {
      if let Some(last) = merged_ranges.last_mut() {
        if range.0 <= last.1 {
          // Overlapping or adjacent, merge
          last.1 = last.1.max(range.1);
        } else {
          merged_ranges.push(range);
        }
      } else {
        merged_ranges.push(range);
      }
    }

    // Render the line with highlights

    write!(stdout, "{center_offset_string}")?;
    let mut last_end = 0;

    for (start, end) in merged_ranges {
      // Print unhighlighted text before this highlight
      if start > last_end {
        write!(stdout, "{}", &line[last_end..start])?;
      }

      // Print highlighted text
      execute!(
        stdout,
        SetBackgroundColor(Color::Yellow),
        SetForegroundColor(Color::Black)
      )?;
      write!(stdout, "{}", &line[start..end])?;
      execute!(stdout, ResetColor)?;

      last_end = end;
    }

    // Print remaining unhighlighted text
    if last_end < line.len() {
      write!(stdout, "{}", &line[last_end..])?;
    }

    // Clear to end of line to match normal view rendering
    execute!(stdout, Clear(ClearType::UntilNewLine))?;

    Ok(true)
  }
}
