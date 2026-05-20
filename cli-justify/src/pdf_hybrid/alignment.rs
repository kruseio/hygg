use crate::text_utils::{char_len, is_ascii_numeric};
use std::collections::HashMap;

use super::structure::{AlignedTocRow, looks_like_toc_section_marker};

pub(super) struct TocAlignmentState {
  title_column_by_prefix_width: HashMap<usize, usize>,
  canonical_numeric_toc_layout: Option<(usize, usize)>,
}

fn set_marker_gap(entry_prefix: &mut String, marker: &str, gap_width: usize) {
  *entry_prefix = format!("{marker}{}", " ".repeat(gap_width));
}

impl TocAlignmentState {
  pub(super) fn new() -> Self {
    Self {
      title_column_by_prefix_width: HashMap::new(),
      canonical_numeric_toc_layout: None,
    }
  }

  fn normalize_section_title_indent(
    &self,
    row: &mut AlignedTocRow,
    allow_fallback_shift: bool,
  ) {
    if let Some((_, section_title_column)) = self.canonical_numeric_toc_layout {
      let target_indent = section_title_column.saturating_sub(1);
      let current_indent = char_len(&row.indent);
      if current_indent.abs_diff(target_indent) <= 1 {
        row.indent = " ".repeat(target_indent);
      }
      return;
    }

    if allow_fallback_shift {
      // The first chapter heading can be extracted one column too far right
      // before subsection rows establish canonical TOC alignment.
      let current_indent = char_len(&row.indent);
      if current_indent >= 21 {
        row.indent = " ".repeat(current_indent - 1);
      }
    }
  }

  pub(super) fn normalize_row(&mut self, row: &mut AlignedTocRow) {
    if is_chapter_like_toc_heading(row) {
      self.normalize_section_title_indent(row, true);
      return;
    }

    if row.entry_prefix.trim().is_empty() {
      self.normalize_section_title_indent(row, false);
      return;
    }

    if let Some(plate_marker) = plate_entry_marker(&row.entry_prefix) {
      let indent_width = char_len(&row.indent);
      let marker_width = char_len(&plate_marker);
      let target_title_column =
        if indent_width <= 3 { 14usize } else { 21usize };
      let target_gap_width =
        target_title_column.saturating_sub(indent_width + marker_width + 1);
      if target_gap_width >= 2 {
        set_marker_gap(&mut row.entry_prefix, &plate_marker, target_gap_width);
      }
      return;
    }

    self.normalize_section_row(row);
  }

  fn normalize_section_row(&mut self, row: &mut AlignedTocRow) {
    let Some(marker) =
      toc_section_marker(&row.entry_prefix).map(str::to_string)
    else {
      return;
    };
    let is_numeric_marker = is_numeric_toc_section_marker(&marker);
    let is_numeric_or_appendix_marker =
      is_numeric_marker || is_appendix_subsection_marker(&marker);

    let mut current_indent = char_len(&row.indent);
    let marker_width = char_len(&marker);
    let dot_offset =
      marker.chars().position(|ch| ch == '.').map_or(0, |idx| idx + 1);
    let mut gap_width =
      char_len(&row.entry_prefix).saturating_sub(marker_width);

    if gap_width == 0 {
      gap_width = 1;
      row.entry_prefix.push(' ');
    }

    let mut current_title_column =
      current_indent + marker_width + gap_width + 1;
    if is_numeric_or_appendix_marker && current_indent <= 3 {
      let target_dot_column = 5usize;
      let target_indent = target_dot_column.saturating_sub(dot_offset);
      if current_indent.abs_diff(target_indent) <= 1 {
        let shift = target_indent.abs_diff(current_indent);
        current_indent = target_indent;
        row.indent = " ".repeat(current_indent);
        if gap_width > shift {
          gap_width -= shift;
          set_marker_gap(&mut row.entry_prefix, &marker, gap_width);
        }
        current_title_column = current_indent + marker_width + gap_width + 1;
      }

      let target_title_column = 14usize;
      let compact_target_gap =
        target_title_column.saturating_sub(current_indent + marker_width + 1);
      if compact_target_gap >= 1 && gap_width.abs_diff(compact_target_gap) <= 2
      {
        gap_width = compact_target_gap;
        set_marker_gap(&mut row.entry_prefix, &marker, gap_width);
        current_title_column = current_indent + marker_width + gap_width + 1;
      }
    }

    if is_numeric_marker
      && dot_offset == 2
      && current_title_column == 22
      && current_indent > 0
    {
      current_indent -= 1;
      row.indent = " ".repeat(current_indent);
      current_title_column -= 1;
    }

    if is_numeric_marker {
      let current_dot_column = current_indent + dot_offset;
      update_numeric_toc_layout(
        &mut self.canonical_numeric_toc_layout,
        current_dot_column,
        current_title_column,
      );
    }

    if is_numeric_or_appendix_marker
      && let Some((target_dot_column, target_title_column)) =
        self.canonical_numeric_toc_layout
    {
      let target_indent = target_dot_column.saturating_sub(dot_offset);
      if current_indent.abs_diff(target_indent) <= 2 {
        current_indent = target_indent;
        row.indent = " ".repeat(current_indent);
      }

      let target_gap_width =
        target_title_column.saturating_sub(current_indent + marker_width + 1);
      if target_gap_width >= 1 && gap_width.abs_diff(target_gap_width) <= 2 {
        gap_width = target_gap_width;
        set_marker_gap(&mut row.entry_prefix, &marker, gap_width);
      }
    }

    if is_numeric_or_appendix_marker && current_indent <= 3 {
      return;
    }

    let prefix_width = char_len(&row.entry_prefix);
    let current_title_column = current_indent + prefix_width;
    let canonical_title_column = self
      .title_column_by_prefix_width
      .entry(prefix_width)
      .or_insert(current_title_column);

    if current_title_column > *canonical_title_column {
      *canonical_title_column = current_title_column;
      return;
    }

    let drift = (*canonical_title_column).saturating_sub(current_title_column);
    if drift > 0 && drift <= 1 {
      let target_indent_width =
        (*canonical_title_column).saturating_sub(prefix_width);
      row.indent = " ".repeat(target_indent_width);
    }
  }
}

pub(super) fn is_chapter_like_toc_heading(row: &AlignedTocRow) -> bool {
  let prefix = row.entry_prefix.trim_start();
  prefix.starts_with("Chapter ") || prefix.starts_with("Appendix ")
}

fn toc_section_marker(entry_prefix: &str) -> Option<&str> {
  let marker = entry_prefix.trim_end().split_whitespace().next()?;
  looks_like_toc_section_marker(marker).then_some(marker)
}

fn is_numeric_toc_section_marker(marker: &str) -> bool {
  let Some((left, right)) = marker.split_once('.') else {
    return false;
  };

  !left.is_empty()
    && !right.is_empty()
    && is_ascii_numeric(left)
    && is_ascii_numeric(right)
}

fn is_appendix_subsection_marker(marker: &str) -> bool {
  let Some((left, right)) = marker.split_once('.') else {
    return false;
  };

  left.len() == 1
    && left.chars().all(|ch| ch.is_ascii_uppercase())
    && !right.is_empty()
    && is_ascii_numeric(right)
}

fn plate_entry_marker(entry_prefix: &str) -> Option<String> {
  let mut parts = entry_prefix.trim_end().split_whitespace();
  let kind = parts.next()?;
  let number = parts.next()?;
  if kind != "Plate" || !is_ascii_numeric(number) {
    return None;
  }

  Some(format!("Plate {number}"))
}

fn update_numeric_toc_layout(
  canonical_numeric_section_layout: &mut Option<(usize, usize)>,
  dot_column: usize,
  title_column: usize,
) {
  match canonical_numeric_section_layout {
    Some((best_dot_column, best_title_column)) => {
      if title_column < *best_title_column
        || (title_column == *best_title_column && dot_column < *best_dot_column)
      {
        *best_dot_column = dot_column;
        *best_title_column = title_column;
      }
    }
    None => {
      *canonical_numeric_section_layout = Some((dot_column, title_column));
    }
  }
}
