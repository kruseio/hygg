mod layout_headings;
mod layout_signals;
mod lists;
mod toc;
mod toc_patterns;

use self::layout_headings::{
  is_centered_short_heading, looks_like_left_aligned_section_heading,
  looks_like_multi_column_row, looks_like_numbered_label_heading,
  looks_like_page_header_or_footer, looks_like_single_word_section_heading,
};
pub(super) use layout_signals::{
  looks_like_code_block_line, looks_like_command_prompt_line,
  looks_like_git_log_graph_line, looks_like_toc_entry,
};
pub(super) use lists::{
  is_list_continuation_line, looks_like_table_or_figure_caption,
  parse_list_marker, should_start_new_pdf_paragraph,
};
pub(super) use toc::{
  AlignedTocRow, AlignedTocRowStart, normalize_preserved_compact_layout_line,
  parse_aligned_toc_continuation, parse_aligned_toc_row_start,
  parse_plain_aligned_toc_row,
};
pub(super) use toc_patterns::looks_like_toc_section_marker;

pub(crate) fn should_keep_pdf_line_layout(line: &str) -> bool {
  let trimmed = line.trim();
  if trimmed.is_empty() {
    return false;
  }

  line.contains('\t')
    || looks_like_toc_entry(trimmed)
    || is_centered_short_heading(line)
    || looks_like_left_aligned_section_heading(line)
    || looks_like_single_word_section_heading(line)
    || looks_like_numbered_label_heading(line)
    || looks_like_page_header_or_footer(line)
    || looks_like_multi_column_row(line)
    || looks_like_code_block_line(line)
}
