use cli_pdf_to_text::PdfLineKind;

mod highlight_tests;
mod narration_tests;
mod skip_tests;
mod span_tests;

pub(super) fn text_kinds(n: usize) -> Vec<PdfLineKind> {
  vec![PdfLineKind::Text; n]
}
