use crate::text_utils::leading_whitespace;

use crate::pdf_hybrid::engine::{FormatterEngine, PendingPdfBlock};
use crate::pdf_hybrid::structure::{
  is_list_continuation_line, looks_like_git_log_graph_line,
  looks_like_table_or_figure_caption, parse_list_marker,
  should_start_new_pdf_paragraph,
};
use crate::pdf_hybrid::wrapping::{
  flush_pending_pdf_block, pending_block_ends_with_hyphen,
  pending_paragraph_ends_mid_sentence,
};

use super::sibling_blanks::{
  drop_trailing_blanks_after_sibling_list, out_ends_in_caption_context,
};

impl FormatterEngine {
  fn start_pending_pdf_block(&mut self, block: PendingPdfBlock) {
    self.close_code_block_and_clear_parent_indent();
    self.begin_preserved_layout_scope();
    self.pending_code_block_parent_callout_indent = None;
    self.pending = Some(block);
  }

  pub(crate) fn handle_list_item_start(&mut self, line: &str) -> bool {
    let Some((indent, marker, content)) = parse_list_marker(line) else {
      return false;
    };

    drop_trailing_blanks_after_sibling_list(&mut self.out, &indent, &marker);

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

  pub(crate) fn handle_list_item_continuation(&mut self, line: &str) -> bool {
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

  pub(crate) fn handle_blank_line(&mut self) -> bool {
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
    // Drop blank lines that fall inside a git-log --graph block. The lopdf
    // backend emits "\n\n" between pages, which splits a multi-page graph
    // into two pieces; without this, downstream output ends up with a stray
    // blank between adjacent graph rows. If a real prose line follows, the
    // code-to-prose transition in `handle_paragraph_line` re-inserts a
    // single blank, so this only collapses spurious mid-block breaks.
    if self.in_code_block
      && self
        .out
        .last()
        .is_some_and(|last| looks_like_git_log_graph_line(last.trim()))
    {
      return true;
    }
    self.out.push(String::new());
    true
  }

  pub(crate) fn handle_paragraph_line(&mut self, line: &str) {
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
        let starts_caption = looks_like_table_or_figure_caption(line.trim());
        let prior_was_list_item =
          matches!(self.pending, Some(PendingPdfBlock::ListItem { .. }));
        let prior_was_caption_pending = match self.pending.as_ref() {
          Some(PendingPdfBlock::Paragraph { lines, .. }) => lines
            .first()
            .map(|first| looks_like_table_or_figure_caption(first.trim()))
            .unwrap_or(false),
          _ => false,
        };
        // When the prior caption was already flushed (page break landed
        // a blank between two adjacent captions in the source, or the
        // prior caption took the preserved-layout path because it had
        // wide internal gaps), the trailing blank in `self.out` is
        // spurious — drop it before recording this caption so the list
        // stays adjacent.
        let prior_was_caption_flushed = starts_caption
          && self.pending.is_none()
          && out_ends_in_caption_context(&mut self.out);
        let prior_was_caption =
          prior_was_caption_pending || prior_was_caption_flushed;
        self.start_pending_pdf_block(PendingPdfBlock::Paragraph {
          indent: leading_whitespace(line).to_string(),
          lines: vec![line.to_string()],
        });
        // Push an explicit blank separator between the just-flushed block
        // and this new paragraph in two cases that pdf_extract leaves
        // ambiguous:
        //   * After a list (option / spec table) ends and prose resumes. The
        //     PDF has extra leading after the last row, but no blank line, so
        //     the table and the next sentence would otherwise run together.
        //   * Before a caption that follows prose — captions read as their own
        //     paragraph in the PDF but the extracted text has them glued to the
        //     trailing sentence above. Consecutive captions (a list of Plate /
        //     Figure / Table entries) must stay adjacent: we already broke them
        //     into separate paragraphs above, but a blank between each one
        //     would turn the list into a sparse double-spaced block.
        // Skip when the prior content is already followed by a blank
        // (handle_blank_line ran, or code-block padding fired) so we
        // don't double up.
        let caption_after_prose = starts_caption && !prior_was_caption;
        if (prior_was_list_item || caption_after_prose)
          && self.out.last().is_some_and(|last| !last.is_empty())
        {
          self.out.push(String::new());
        }
      }
    }
  }
}
