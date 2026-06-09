mod has_highlights;
mod render_combined;
mod render_combined_buffered;
mod render_persistent;

use cli_pdf_to_text::PdfLineKind;

use super::core::Editor;

#[derive(Debug, Clone, Copy)]
pub(crate) enum HighlightType {
  Selection,
  Persistent,
}

impl Editor {
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
}
