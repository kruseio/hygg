#[cfg(test)]
mod tests {
  use super::*;
  use crate::editor::core::{BufferState, Editor, EditorMode};
  use cli_pdf_to_text::PdfLineKind;

  #[test]
  fn test_toggle_highlight_with_selection() {
    let lines = vec![
      "First line".to_string(),
      "Second line to highlight".to_string(),
      "Third line".to_string(),
    ];

    let mut editor = Editor::new(lines, 80);

    // Simulate visual mode selection
    editor.set_active_mode(EditorMode::VisualChar);

    // Set selection in editor state
    editor.editor_state.selection_start = Some((1, 7)); // "line"
    editor.editor_state.selection_end = Some((1, 11)); // "line"
    editor.editor_state.visual_selection_active = true;
    editor.editor_state.previous_visual_mode = Some(EditorMode::VisualChar);

    // Also set in buffer state
    if let Some(buffer) = editor.buffers.get_mut(0) {
      buffer.selection_start = Some((1, 7));
      buffer.selection_end = Some((1, 11));
    }

    // Toggle highlight
    editor.toggle_highlight();

    // Check that a highlight was added
    assert_eq!(editor.highlights.highlights.len(), 1);

    let highlight = &editor.highlights.highlights[0];
    // "First line\n" = 11 chars, then position 7 in second line
    assert_eq!(highlight.start, 11 + 7); // 18
    assert_eq!(highlight.end, 11 + 12); // 23 (inclusive end)
  }

  #[test]
  fn highlight_offsets_skip_ansi_art_rows() {
    let lines = vec![
      "First line".to_string(),
      "\x1b[38;2;1;2;3m▀\x1b[0m".to_string(),
      "Third line".to_string(),
    ];

    let mut editor = Editor::new(lines, 80);
    editor.line_kinds =
      vec![PdfLineKind::Text, PdfLineKind::AnsiArt, PdfLineKind::Text];
    editor.buffers[0].line_kinds = editor.line_kinds.clone();
    editor.set_active_mode(EditorMode::VisualChar);
    editor.editor_state.selection_start = Some((2, 0));
    editor.editor_state.selection_end = Some((2, 4));
    editor.editor_state.visual_selection_active = true;
    editor.editor_state.previous_visual_mode = Some(EditorMode::VisualChar);

    if let Some(buffer) = editor.buffers.get_mut(0) {
      buffer.selection_start = Some((2, 0));
      buffer.selection_end = Some((2, 4));
    }

    editor.toggle_highlight();

    assert_eq!(editor.highlights.highlights.len(), 1);
    let highlight = &editor.highlights.highlights[0];
    assert_eq!(highlight.start, 11);
    assert_eq!(highlight.end, 16);
  }
}
