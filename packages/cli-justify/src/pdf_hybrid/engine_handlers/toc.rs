use crate::pdf_hybrid::alignment::is_chapter_like_toc_heading;
use crate::pdf_hybrid::engine::{FormatterEngine, PendingAlignedTocRow};
use crate::pdf_hybrid::structure::{
  AlignedTocRow, parse_aligned_toc_continuation, parse_aligned_toc_row_start,
  parse_plain_aligned_toc_row,
};
use crate::pdf_hybrid::wrapping::{
  flush_pending_aligned_toc_row, wrap_aligned_toc_row,
};

impl FormatterEngine {
  pub(crate) fn handle_aligned_toc_row_start(&mut self, line: &str) -> bool {
    let Some(parsed) = parse_aligned_toc_row_start(line) else {
      return false;
    };

    self.close_code_block_and_clear_parent_indent();
    let _ = self.flush_pending_block_with_margin();
    self.apply_pending_deep_callout_bottom_margin();
    flush_pending_aligned_toc_row(
      &mut self.pending_toc_row,
      &mut self.out,
      self.line_width,
      &mut self.alignment_state,
    );

    if let Some(page_number) = parsed.page_number {
      let mut toc_row = AlignedTocRow {
        indent: parsed.indent,
        entry_prefix: parsed.entry_prefix,
        title: parsed.title_fragment,
        page_number,
      };
      self.alignment_state.normalize_row(&mut toc_row);
      if self.in_aligned_toc
        && is_chapter_like_toc_heading(&toc_row)
        && self.out.last().is_some_and(|last| !last.is_empty())
      {
        self.out.push(String::new());
      }
      self.out.extend(wrap_aligned_toc_row(&toc_row, self.line_width));
    } else {
      self.pending_toc_row = Some(PendingAlignedTocRow {
        indent: parsed.indent,
        entry_prefix: parsed.entry_prefix,
        title: parsed.title_fragment,
      });
    }

    self.in_aligned_toc = true;
    true
  }

  pub(crate) fn handle_pending_aligned_toc_row(&mut self, line: &str) -> bool {
    if self.pending_toc_row.is_none() {
      return false;
    }

    if let Some((fragment, page_number)) = parse_aligned_toc_continuation(line)
    {
      self.close_code_block_and_clear_parent_indent();

      if let Some(pending_row) = self.pending_toc_row.as_mut() {
        if !pending_row.title.is_empty() {
          pending_row.title.push(' ');
        }
        pending_row.title.push_str(fragment.trim());
      }

      if let Some(page_number) = page_number {
        let pending_row = self
          .pending_toc_row
          .take()
          .expect("pending_toc_row exists when finishing TOC row");
        let mut toc_row = AlignedTocRow {
          indent: pending_row.indent,
          entry_prefix: pending_row.entry_prefix,
          title: pending_row.title,
          page_number,
        };
        self.alignment_state.normalize_row(&mut toc_row);
        self.out.extend(wrap_aligned_toc_row(&toc_row, self.line_width));
      }

      self.in_aligned_toc = true;
      return true;
    }

    self.close_code_block_and_clear_parent_indent();
    flush_pending_aligned_toc_row(
      &mut self.pending_toc_row,
      &mut self.out,
      self.line_width,
      &mut self.alignment_state,
    );
    false
  }

  pub(crate) fn handle_plain_aligned_toc_row(&mut self, line: &str) -> bool {
    if !self.in_aligned_toc {
      return false;
    }

    let Some(mut toc_row) = parse_plain_aligned_toc_row(line) else {
      return false;
    };

    self.close_code_block_and_clear_parent_indent();
    self.alignment_state.normalize_row(&mut toc_row);
    self.out.extend(wrap_aligned_toc_row(&toc_row, self.line_width));
    true
  }
}
