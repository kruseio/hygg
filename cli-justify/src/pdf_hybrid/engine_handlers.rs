use crate::text_utils::{char_len, leading_whitespace};

use super::alignment::is_chapter_like_toc_heading;
use super::engine::{FormatterEngine, PendingAlignedTocRow, PendingPdfBlock};
use super::structure::{
  AlignedTocRow, is_list_continuation_line, looks_like_git_log_graph_line,
  looks_like_table_or_figure_caption, parse_aligned_toc_continuation,
  parse_aligned_toc_row_start, parse_list_marker, parse_plain_aligned_toc_row,
  should_start_new_pdf_paragraph,
};
use super::wrapping::{
  flush_pending_aligned_toc_row, flush_pending_pdf_block,
  pending_block_ends_with_hyphen, pending_paragraph_ends_mid_sentence,
  wrap_aligned_toc_row,
};

// How many wrapped lines of one list item / caption we'll scan back through
// to recognise a sibling. Real captions wrap to 2–3 lines; bumping a bit for
// margin without making the scan unbounded.
const MAX_SIBLING_LOOKBACK_LINES: usize = 12;

/// True if `line` is the first line of a list item that shares `indent` and
/// `marker` with a new item we're about to start. Bullet markers must match
/// exactly. Numeric markers (`1.`, `12)`) match by shape — same indent and
/// the same punctuation style — since each item carries a different counter.
fn line_starts_sibling_list_item(
  line: &str,
  indent: &str,
  marker: &str,
) -> bool {
  if line.starts_with(&format!("{indent}{marker}")) {
    return true;
  }
  let Some(rest) = line.strip_prefix(indent) else {
    return false;
  };
  let marker_punct = marker.trim_end().chars().last();
  let Some(marker_punct) = marker_punct else {
    return false;
  };
  if marker_punct != '.' && marker_punct != ')' {
    return false;
  }
  let digit_count = rest.chars().take_while(|ch| ch.is_ascii_digit()).count();
  if digit_count == 0 {
    return false;
  }
  let mut after_digits = rest.chars().skip(digit_count);
  let Some(delim) = after_digits.next() else {
    return false;
  };
  if delim != marker_punct {
    return false;
  }
  matches!(after_digits.next(), Some(' '))
}

/// When a bulleted / numbered list spans a PDF page break the extractor
/// emits one or more blank lines between sibling items. The list reads as
/// one continuous block in the source PDF, so the blanks split it
/// artificially. If the most recently emitted block is a sibling list item,
/// drop the trailing blanks.
fn drop_trailing_blanks_after_sibling_list(
  out: &mut Vec<String>,
  indent: &str,
  marker: &str,
) {
  let mut blanks_start = out.len();
  while blanks_start > 0 && out[blanks_start - 1].is_empty() {
    blanks_start -= 1;
  }
  if blanks_start == out.len() || blanks_start == 0 {
    return;
  }
  let continuation_indent_width = char_len(indent) + char_len(marker);
  let scan_floor = blanks_start.saturating_sub(MAX_SIBLING_LOOKBACK_LINES);
  for idx in (scan_floor..blanks_start).rev() {
    let line = &out[idx];
    if line.is_empty() {
      return;
    }
    if line_starts_sibling_list_item(line, indent, marker) {
      out.truncate(blanks_start);
      return;
    }
    // Continuation lines of the prior list item carry at least the
    // continuation indent. A shorter indent means we've left the prior
    // item without finding its marker line — different block.
    let leading_ws = line.chars().take_while(|ch| *ch == ' ').count();
    if leading_ws < continuation_indent_width {
      return;
    }
  }
}

/// Same idea as `drop_trailing_blanks_after_sibling_list`, but for the
/// front-matter caption lists (`Plate 1 …`, `Figure 3.4 …`, `Table 2 …`).
/// These are emitted as standalone paragraphs, so a page-break blank lands
/// directly between two caption entries and stretches the list out.
///
/// Returns `true` when the most recent flushed content is part of a caption
/// (with or without trailing blanks between it and the next caption). Drops
/// the trailing blanks as a side effect so the captions read as one block.
/// Also reports `true` when the prior caption was emitted via the
/// preserved-layout path (wide internal gaps); without this the paragraph
/// handler treats the new caption as a "prose → caption" transition and
/// inserts a spurious separator blank.
fn out_ends_in_caption_context(out: &mut Vec<String>) -> bool {
  let mut blanks_start = out.len();
  while blanks_start > 0 && out[blanks_start - 1].is_empty() {
    blanks_start -= 1;
  }
  if blanks_start == 0 {
    return false;
  }
  let had_blanks = blanks_start < out.len();
  let scan_floor = blanks_start.saturating_sub(MAX_SIBLING_LOOKBACK_LINES);
  for idx in (scan_floor..blanks_start).rev() {
    let line = &out[idx];
    if line.is_empty() {
      return false;
    }
    if looks_like_table_or_figure_caption(line.trim()) {
      if had_blanks {
        out.truncate(blanks_start);
      }
      return true;
    }
  }
  false
}

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
