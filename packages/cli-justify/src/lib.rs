// Slicing a str at a byte that is not a char boundary panics (hygg issue #99).
// Every slice goes through `str_slice`; with CI's `-D warnings` a raw
// `&s[a..b]` is a build error. Tests may slice.
#![cfg_attr(not(test), warn(clippy::string_slice))]

mod justify_text;
mod pdf_hybrid;
mod str_slice;
mod text_utils;
mod wrap;

pub use justify_text::justify;
pub use pdf_hybrid::{
  PartialParagraph, PdfPageJustified, inter_page_blank_count,
  justify_pdf_hybrid, justify_pdf_page, justify_pdf_seam,
  pdf_hybrid_narration_skip_mask,
};
pub use wrap::wrap_preserve_whitespace;
