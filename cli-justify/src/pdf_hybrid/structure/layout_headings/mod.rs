mod layout_rows;
mod text_headings;

pub(crate) use layout_rows::{
  looks_like_multi_column_row, looks_like_page_header_or_footer,
};
pub(crate) use text_headings::{
  is_centered_short_heading, looks_like_left_aligned_section_heading,
  looks_like_numbered_label_heading, looks_like_single_word_section_heading,
};
