use crate::text_utils::char_len;

use crate::pdf_hybrid::alignment::TocAlignmentState;
use crate::pdf_hybrid::engine::PendingAlignedTocRow;
use crate::pdf_hybrid::structure::AlignedTocRow;
use crate::pdf_hybrid::wrapping_plain::{
  split_last_word, wrap_plain_with_prefix,
};

pub(crate) fn wrap_aligned_toc_row(
  row: &AlignedTocRow,
  line_width: usize,
) -> Vec<String> {
  let first_prefix = format!("{}{}", row.indent, row.entry_prefix);
  let continuation_prefix = " ".repeat(char_len(&first_prefix));
  let mut wrapped = wrap_plain_with_prefix(
    &row.title,
    line_width,
    &first_prefix,
    &continuation_prefix,
  );

  let page_suffix = format!("   {}", row.page_number);
  let first_limit = line_width.saturating_sub(char_len(&first_prefix));
  let continuation_limit =
    line_width.saturating_sub(char_len(&continuation_prefix));

  while let Some(last_line) = wrapped.last() {
    let last_idx = wrapped.len() - 1;
    let prefix_len = if last_idx == 0 {
      char_len(&first_prefix)
    } else {
      char_len(&continuation_prefix)
    };
    let usable_width =
      if last_idx == 0 { first_limit } else { continuation_limit };
    let last_text = &last_line[prefix_len..];
    let required = char_len(last_text) + char_len(&page_suffix);
    if required <= usable_width {
      break;
    }

    if let Some((head, tail)) = split_last_word(last_text) {
      wrapped[last_idx] = if last_idx == 0 {
        format!("{first_prefix}{head}")
      } else {
        format!("{continuation_prefix}{head}")
      };
      wrapped.push(format!("{continuation_prefix}{tail}"));
    } else {
      wrapped.push(continuation_prefix.clone());
      break;
    }
  }

  if let Some(last_line) = wrapped.last_mut() {
    last_line.push_str(&page_suffix);
  }

  wrapped
}

pub(crate) fn flush_pending_aligned_toc_row(
  pending: &mut Option<PendingAlignedTocRow>,
  out: &mut Vec<String>,
  line_width: usize,
  alignment_state: &mut TocAlignmentState,
) {
  let Some(row) = pending.take() else {
    return;
  };

  let mut row = AlignedTocRow {
    indent: row.indent,
    entry_prefix: row.entry_prefix,
    title: row.title,
    page_number: String::new(),
  };
  alignment_state.normalize_row(&mut row);

  let first_prefix = format!("{}{}", row.indent, row.entry_prefix);
  let continuation_prefix = " ".repeat(char_len(&first_prefix));
  out.extend(wrap_plain_with_prefix(
    &row.title,
    line_width,
    &first_prefix,
    &continuation_prefix,
  ));
}
