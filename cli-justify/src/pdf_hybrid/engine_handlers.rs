use crate::text_utils::leading_whitespace;

use super::alignment::is_chapter_like_toc_heading;
use super::engine::{FormatterEngine, PendingAlignedTocRow, PendingPdfBlock};
use super::structure::{
  AlignedTocRow, is_list_continuation_line, parse_aligned_toc_continuation,
  parse_aligned_toc_row_start, parse_list_marker, parse_plain_aligned_toc_row,
  should_start_new_pdf_paragraph,
};
use super::wrapping::{
  flush_pending_aligned_toc_row, flush_pending_pdf_block,
  pending_block_ends_with_hyphen, pending_paragraph_ends_mid_sentence,
  wrap_aligned_toc_row,
};

impl FormatterEngine {
  fn start_pending_pdf_block(&mut self, block: PendingPdfBlock) {
    self.close_code_block_and_clear_parent_indent();
    self.begin_preserved_layout_scope();
    self.pending_code_block_parent_callout_indent = None;
    self.pending = Some(block);
  }

  pub(super) fn handle_aligned_toc_row_start(&mut self, line: &str) -> bool {
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

  pub(super) fn handle_pending_aligned_toc_row(&mut self, line: &str) -> bool {
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

  pub(super) fn handle_plain_aligned_toc_row(&mut self, line: &str) -> bool {
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

  pub(super) fn handle_list_item_start(&mut self, line: &str) -> bool {
    let Some((indent, marker, content)) = parse_list_marker(line) else {
      return false;
    };

    let mut lines = Vec::new();
    if !content.is_empty() {
      lines.push(content);
    }
    self.start_pending_pdf_block(PendingPdfBlock::ListItem {
      indent,
      marker,
      lines,
    });
    true
  }

  pub(super) fn handle_list_item_continuation(&mut self, line: &str) -> bool {
    if let Some(PendingPdfBlock::ListItem { indent, marker, lines }) =
      self.pending.as_mut()
      && is_list_continuation_line(line, indent, marker)
    {
      self.in_aligned_toc = false;
      self.pending_code_block_parent_callout_indent = None;
      lines.push(line.trim().to_string());
      return true;
    }

    false
  }

  pub(super) fn handle_blank_line(&mut self) -> bool {
    if self.in_aligned_toc {
      return true;
    }

    self.in_aligned_toc = false;
    if pending_block_ends_with_hyphen(&self.pending) {
      return true;
    }
    if pending_paragraph_ends_mid_sentence(&self.pending) {
      return true;
    }
    if let Some(capped_indent) =
      flush_pending_pdf_block(&mut self.pending, &mut self.out, self.line_width)
    {
      self.pending_code_block_parent_callout_indent = Some(capped_indent);
    }
    self.pending_deep_callout_bottom_margin = false;
    self.out.push(String::new());
    true
  }

  pub(super) fn handle_paragraph_line(&mut self, line: &str) {
    self.close_code_block_and_clear_parent_indent();

    match self.pending.as_mut() {
      Some(PendingPdfBlock::Paragraph { indent, lines })
        if !should_start_new_pdf_paragraph(
          indent,
          lines.last().map(String::as_str).unwrap_or_default(),
          line,
        ) =>
      {
        self.in_aligned_toc = false;
        lines.push(line.to_string());
      }
      _ => {
        self.start_pending_pdf_block(PendingPdfBlock::Paragraph {
          indent: leading_whitespace(line).to_string(),
          lines: vec![line.to_string()],
        });
      }
    }
  }
}
