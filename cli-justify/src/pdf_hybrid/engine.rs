use super::alignment::TocAlignmentState;
use super::figure_labels::normalize_sparse_figure_label_blocks;
use super::wrapping::{flush_pending_aligned_toc_row, flush_pending_pdf_block};

pub(super) enum PendingPdfBlock {
  Paragraph { indent: String, lines: Vec<String> },
  ListItem { indent: String, marker: String, lines: Vec<String> },
}

pub(super) struct PendingAlignedTocRow {
  pub(super) indent: String,
  pub(super) entry_prefix: String,
  pub(super) title: String,
}

pub(super) struct FormatterEngine {
  pub(super) line_width: usize,
  pub(super) out: Vec<String>,
  pub(super) pending: Option<PendingPdfBlock>,
  pub(super) pending_toc_row: Option<PendingAlignedTocRow>,
  pub(super) in_aligned_toc: bool,
  pub(super) in_code_block: bool,
  pub(super) alignment_state: TocAlignmentState,
  pub(super) shell_session_indent: Option<String>,
  pub(super) pending_deep_callout_bottom_margin: bool,
  pub(super) pending_code_block_parent_callout_indent: Option<usize>,
  pub(super) code_block_source_base_indent: Option<usize>,
  pub(super) code_block_target_base_indent: Option<usize>,
}

impl FormatterEngine {
  pub(super) fn new(line_width: usize) -> Self {
    Self {
      line_width,
      out: Vec::new(),
      pending: None,
      pending_toc_row: None,
      in_aligned_toc: false,
      in_code_block: false,
      alignment_state: TocAlignmentState::new(),
      shell_session_indent: None,
      pending_deep_callout_bottom_margin: false,
      pending_code_block_parent_callout_indent: None,
      code_block_source_base_indent: None,
      code_block_target_base_indent: None,
    }
  }

  pub(super) fn process_line(&mut self, line: &str) {
    if self.pending_deep_callout_bottom_margin && !line.trim().is_empty() {
      self.apply_pending_deep_callout_bottom_margin();
    }

    if self.handle_aligned_toc_row_start(line)
      || self.handle_pending_aligned_toc_row(line)
      || self.handle_plain_aligned_toc_row(line)
      || self.handle_shell_session_line(line)
      || self.handle_list_item_start(line)
      || self.handle_list_item_continuation(line)
      || line.trim().is_empty() && self.handle_blank_line()
      || self.handle_preserved_pdf_layout_line(line)
    {
      return;
    }

    self.handle_paragraph_line(line);
  }

  pub(super) fn finish(mut self) -> Vec<String> {
    flush_pending_aligned_toc_row(
      &mut self.pending_toc_row,
      &mut self.out,
      self.line_width,
      &mut self.alignment_state,
    );
    let _ = flush_pending_pdf_block(
      &mut self.pending,
      &mut self.out,
      self.line_width,
    );
    normalize_sparse_figure_label_blocks(&mut self.out, self.line_width);
    self.out
  }
}

pub fn justify_pdf_hybrid(text: &str, line_width: usize) -> Vec<String> {
  let mut engine = FormatterEngine::new(line_width);
  for line in text.split('\n') {
    engine.process_line(line);
  }
  engine.finish()
}
