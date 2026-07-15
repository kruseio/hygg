mod hyphenation;
mod paragraph;
mod toc;

pub(crate) use paragraph::{
  flush_pending_pdf_block, pending_block_ends_with_hyphen,
  pending_paragraph_ends_mid_sentence,
};
pub(crate) use toc::{flush_pending_aligned_toc_row, wrap_aligned_toc_row};
