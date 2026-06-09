#[cfg(feature = "pdf-rendering")]
use crate::stream::images::render_dynamic_image_region;
use crate::stream::types::{VisualImageRows, VisualTextRow};
#[cfg(feature = "pdf-rendering")]
use crate::stream::vector_detect::detect_vector_diagram_regions;

#[cfg(feature = "pdf-rendering")]
pub(crate) fn render_vector_diagram_regions(
  doc: &pdf_oxide::PdfDocument,
  page_0based: usize,
  col: usize,
  native_rows: &[VisualTextRow],
  allow_missing_native_text: bool,
) -> Vec<VisualImageRows> {
  if col == 0 {
    return Vec::new();
  }

  let (page_left, page_top, page_width, page_height) =
    page_metrics(doc, page_0based);
  let paths = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    doc.extract_paths(page_0based)
  }))
  .ok()
  .and_then(Result::ok)
  .unwrap_or_default();
  let regions = detect_vector_diagram_regions(
    &paths,
    page_left,
    page_top,
    page_width,
    page_height,
    native_rows,
    allow_missing_native_text,
  );

  let options = pdf_oxide::rendering::RenderOptions::with_dpi(120);
  let mut out = Vec::new();
  for region in regions {
    let rendered = pdf_oxide::rendering::render_page_region(
      doc,
      page_0based,
      (region.left, region.bottom, region.width, region.height),
      &options,
    );
    let Ok(rendered) = rendered else {
      continue;
    };
    let Ok(dynamic_image) = image::load_from_memory(&rendered.data) else {
      continue;
    };
    if let Some(rows) = render_dynamic_image_region(
      &dynamic_image,
      region,
      page_left,
      page_width,
      col,
    ) {
      out.push(rows);
    }
  }
  out
}

#[cfg(any(feature = "pdf-rendering", feature = "pdf-ocr-bundled"))]
pub(crate) fn page_metrics(
  doc: &pdf_oxide::PdfDocument,
  page_0based: usize,
) -> (f32, f32, f32, f32) {
  doc
    .get_page_media_box(page_0based)
    .ok()
    .map(|(llx, lly, urx, ury)| {
      (llx.min(urx), lly.min(ury), (urx - llx).abs(), (ury - lly).abs())
    })
    .filter(|(_, _, w, h)| *w > 0.0 && *h > 0.0)
    .unwrap_or((0.0, 0.0, 612.0, 792.0))
}

#[cfg(not(feature = "pdf-rendering"))]
pub(crate) fn render_vector_diagram_regions(
  _doc: &pdf_oxide::PdfDocument,
  _page_0based: usize,
  _col: usize,
  _native_rows: &[VisualTextRow],
  _allow_missing_native_text: bool,
) -> Vec<VisualImageRows> {
  Vec::new()
}
