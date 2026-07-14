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
  // Page 1's contribution to the flat buffer, counted the way the streaming
  // render does (`rendered_line_count`, images and separator included) — the
  // same space a flat offset lives in. progit's page 1 is a full-page cover
  // image, so a text-only `standalone_lines` count would be far smaller and
  // put the page-2 boundary at the wrong flat line.
  let load = |page: usize| {
    stream
      .extract_page_with_images(page, 80)
      .map(|r| crate::editor::streaming::LoadedPage::from_rendered(r, 80))
  };
  let page_1 = load(1);
  let page_2 = load(2);
  let first_page_lines = page_1
    .as_ref()
    .map(|p| p.rendered_line_count(None, page_2.as_ref(), false, 80))
    .unwrap_or(1)
    .max(1);

  assert_eq!(infer_pdf_position_from_flat_offset(&stream, 0, 80), Some((1, 0)));
  assert_eq!(
    infer_pdf_position_from_flat_offset(&stream, first_page_lines, 80),
    Some((2, 0))
  );
}
