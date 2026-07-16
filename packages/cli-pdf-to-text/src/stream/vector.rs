#[cfg(feature = "pdf-rendering")]
use crate::stream::images::render_dynamic_image_region;
#[cfg(feature = "pdf-rendering")]
use crate::stream::types::PdfRegion;
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
    if region_raster_is_oversized(&region) {
      continue;
    }
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

/// Would rasterizing this region at 120 DPI ask for an unreasonable image?
///
/// A detected region is clamped to the page's media box and nothing else, and
/// the media box is a number in the document. At 120 DPI each point becomes
/// 1.67 px and each pixel four bytes, so the 14400pt page the PDF spec permits
/// rasterizes to 24000 x 24000 — 2.3 GB, for a "diagram". 64 megapixels is
/// ~256 MB and still far past anything real: a letter page is 1.4k x 1.8k px,
/// 2.6 MP, and a region is only ever a part of one. Whatever survives is about
/// to be downsampled to `col` terminal cells regardless.
#[cfg(feature = "pdf-rendering")]
fn region_raster_is_oversized(region: &PdfRegion) -> bool {
  const MAX_REGION_PIXELS: f32 = 64_000_000.0;
  const SCALE: f32 = 120.0 / 72.0;

  let pixels = (region.width * SCALE) * (region.height * SCALE);
  !pixels.is_finite() || pixels > MAX_REGION_PIXELS
}

#[cfg(any(feature = "pdf-rendering", feature = "ocr"))]
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
