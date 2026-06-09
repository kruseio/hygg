use super::pdf::{infer_pdf_position_from_flat_offset, pdf_preload_radius};

#[test]
fn ocr_pdf_streams_with_smaller_initial_preload() {
  assert_eq!(pdf_preload_radius(false), 10);
  assert_eq!(pdf_preload_radius(true), 0);
}

#[test]
fn flat_offset_restore_can_infer_later_pdf_page() {
  let pdf_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../test-data/pdf/progit-1-50.pdf");
  if !pdf_path.exists() {
    return;
  }
  let stream = cli_pdf_to_text::PdfStream::open(
    pdf_path.to_str().expect("test path should be utf-8"),
  )
  .expect("test PDF should open");
  let first_page_lines = stream
    .extract_page(1)
    .map(|raw| {
      crate::editor::streaming::LoadedPage::from_raw(raw, 80)
        .standalone_lines
        .len()
        .max(1)
    })
    .unwrap_or(1);

  assert_eq!(infer_pdf_position_from_flat_offset(&stream, 0, 80), Some((1, 0)));
  assert_eq!(
    infer_pdf_position_from_flat_offset(&stream, first_page_lines, 80),
    Some((2, 0))
  );
}
