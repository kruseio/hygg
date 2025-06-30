use crossterm::{
  execute,
  style::{Color, ResetColor, SetBackgroundColor, SetForegroundColor},
};
use std::io::{Result as IoResult, Write};

use super::core::{Editor, EditorMode};

#[derive(Debug, Clone, Copy)]
pub(super) enum HighlightType {
  Selection,
  Persistent,
}

impl Editor {
  // Highlight persistent text highlights
  pub fn highlight_persistent(
    &self,
    stdout: &mut impl Write,
    line_index: usize,
    line: &str,
    center_offset_string: &str,
  ) -> IoResult<bool> {
    let current_line_idx = self.offset + line_index;

    // Calculate absolute position range for this line
    let mut abs_line_start = 0;
    for i in 0..current_line_idx {
      if i < self.lines.len() {
        abs_line_start += self.lines[i].len() + 1; // +1 for newline
      }
    }
    let abs_line_end = abs_line_start + line.len();

    // Get highlights that overlap with this line
    let line_highlights =
      self.highlights.get_highlights_for_range(abs_line_start, abs_line_end);

    if line_highlights.is_empty() {
      return Ok(false);
    }

    self.debug_log(&format!(
      "Rendering {} highlights for line {} (abs range: {}-{})",
      line_highlights.len(),
      current_line_idx,
      abs_line_start,
      abs_line_end
    ));

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
    execute!(
      stdout,
      crossterm::terminal::Clear(crossterm::terminal::ClearType::UntilNewLine)
    )?;

    Ok(true)
  }

  // Check if a line has persistent highlights
  pub fn has_persistent_highlights_on_line(&self, line_index: usize) -> bool {
    let current_line_idx = self.offset + line_index;

    // Calculate absolute position range for this line
    let mut abs_line_start = 0;
    for i in 0..current_line_idx {
      if i < self.lines.len() {
        abs_line_start += self.lines[i].len() + 1;
      }
    }

    let abs_line_end = if current_line_idx < self.lines.len() {
      abs_line_start + self.lines[current_line_idx].len()
    } else {
      abs_line_start
    };

    // Check if any highlights overlap with this line
    !self
      .highlights
      .get_highlights_for_range(abs_line_start, abs_line_end)
      .is_empty()
  }

  // Render combined highlights (visual selection + persistent highlights)
  pub fn render_combined_highlights(
    &self,
    stdout: &mut impl Write,
    line_index: usize,
    line: &str,
    center_offset_string: &str,
  ) -> IoResult<bool> {
    let current_line_idx = self.offset + line_index;

    // Get all highlight ranges for this line
    let mut ranges: Vec<(usize, usize, HighlightType)> = Vec::new();

    // Add visual selection range if present
    if let (Some(start), Some(end)) =
      (self.editor_state.selection_start, self.editor_state.selection_end)
    {
      let is_line_mode = self.editor_state.mode == EditorMode::VisualLine
        || (self.editor_state.visual_selection_active
          && self.editor_state.previous_visual_mode
            == Some(EditorMode::VisualLine));

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
    let mut abs_line_start = 0;
    for i in 0..current_line_idx {
      if i < self.lines.len() {
        abs_line_start += self.lines[i].len() + 1;
      }
    }
    let abs_line_end = abs_line_start + line.len();

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
    write!(stdout, "{center_offset_string}")?;
    let mut last_end = 0;

    for (start, end, highlight_type) in ranges {
      // Print unhighlighted text before this highlight
      if start > last_end {
        write!(stdout, "{}", &line[last_end..start])?;
      }

      // Print highlighted text with appropriate style
      match highlight_type {
        HighlightType::Selection => {
          execute!(
            stdout,
            SetBackgroundColor(Color::DarkBlue),
            SetForegroundColor(Color::White)
          )?;
        }
        HighlightType::Persistent => {
          execute!(
            stdout,
            SetBackgroundColor(Color::Yellow),
            SetForegroundColor(Color::Black)
          )?;
        }
      }

      // Handle overlapping ranges - use the max end
      let actual_end = if last_end > start { last_end.max(end) } else { end };
      let actual_start = last_end.max(start);

      if actual_start < actual_end && actual_start < line.len() {
        write!(stdout, "{}", &line[actual_start..actual_end.min(line.len())])?;
      }

      execute!(stdout, ResetColor)?;
      last_end = actual_end;
    }

    // Print remaining unhighlighted text
    if last_end < line.len() {
      write!(stdout, "{}", &line[last_end..])?;
    }
    execute!(
      stdout,
      crossterm::terminal::Clear(crossterm::terminal::ClearType::UntilNewLine)
    )?;

    Ok(true)
  }
}