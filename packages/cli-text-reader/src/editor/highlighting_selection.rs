use super::char_boundary::ByteCol;
use super::core::{Editor, EditorMode};
use super::highlighting::LineView;

impl Editor {
  // The visual selection's byte columns on document line `line_idx` (text
  // `line`) as drawn in `view`, or `None` if no visible selection covers the
  // line. The columns may be stale; `line_highlights` settles them onto the
  // line.
  //
  // A selection is visible while its buffer is in a visual mode, or — for the
  // active buffer — while `visual_selection_active` keeps it on screen after
  // `:` from visual mode. It covers whole lines when it was made in VisualLine,
  // including in that command-mode case (`previous_visual_mode`).
  pub(crate) fn selection_columns(
    &self,
    view: LineView,
    line_idx: usize,
    line: &str,
  ) -> Option<(ByteCol, ByteCol)> {
    let (start, end, mode, kept, previous) = match view {
      LineView::Pane(idx) if idx != self.active_buffer => {
        let buffer = self.buffers.get(idx)?;
        (
          buffer.selection_start?,
          buffer.selection_end?,
          &buffer.mode,
          false,
          None,
        )
      }
      _ => {
        let state = &self.editor_state;
        (
          state.selection_start?,
          state.selection_end?,
          &state.mode,
          state.visual_selection_active,
          state.previous_visual_mode.as_ref(),
        )
      }
    };

    let in_visual_mode =
      matches!(mode, EditorMode::VisualChar | EditorMode::VisualLine);
    if !in_visual_mode && !kept {
      return None;
    }
    let line_mode = *mode == EditorMode::VisualLine
      || (kept && previous == Some(&EditorMode::VisualLine));

    // Order the ends so `first` comes first (by line, then column).
    let (first, last) = (start.min(end), start.max(end));
    if line_idx < first.line || line_idx > last.line {
      return None;
    }
    let (start, end) = (ByteCol::ZERO, ByteCol::end(line));
    let (first_col, last_col) = (first.col, last.col);
    Some(if line_mode {
      (start, end)
    } else if first.line == last.line {
      (first_col, last_col)
    } else if line_idx == first.line {
      (first_col, end)
    } else if line_idx == last.line {
      (start, last_col)
    } else {
      (start, end)
    })
  }
}
