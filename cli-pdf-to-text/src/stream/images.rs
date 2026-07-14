use cli_image_to_ascii::{RenderConfig, render_half_block};

use crate::stream::geometry::{
  pdf_image_height_rows, pdf_width_to_cells, pdf_x_to_cells,
};
use crate::stream::types::{PdfRegion, VisualImageRows};

pub(crate) fn render_pdf_images(
  doc: &pdf_oxide::PdfDocument,
  page_0based: usize,
  col: usize,
  images: &[pdf_oxide::extractors::PdfImage],
) -> Vec<VisualImageRows> {
  render_pdf_images_sourced(doc, page_0based, col, images)
    .into_iter()
    .map(|(rows, _)| rows)
    .collect()
}

/// Like [`render_pdf_images`] but also returns the full-resolution decoded
/// source image behind each ASCII-art block, in the same order/filtering (so a
/// GUI can render a crisp raster in place of the half-block art without moving
/// anything). The ASCII-art path stays authoritative for what actually lands in
/// the composed lines — this only hands back the extra pixels alongside.
pub(crate) fn render_pdf_images_sourced(
  doc: &pdf_oxide::PdfDocument,
  page_0based: usize,
  col: usize,
  images: &[pdf_oxide::extractors::PdfImage],
) -> Vec<(VisualImageRows, image::DynamicImage)> {
  if col == 0 {
    return Vec::new();
  }
  let (page_left, page_width) = doc
    .get_page_media_box(page_0based)
    .ok()
    .map(|(llx, _, urx, _)| (llx, (urx - llx).abs()))
    .filter(|(_, w)| *w > 0.0)
    .unwrap_or((0.0, 612.0));

  let mut out = Vec::new();
  for image in images {
    let Some(bbox) = image.bbox() else {
      continue;
    };
    if bbox.width <= 0.0 || bbox.height <= 0.0 {
      continue;
    }
    let Ok(dynamic_image) = image.to_dynamic_image() else {
      continue;
    };
    if let Some(rows) = render_dynamic_image_region(
      &dynamic_image,
      PdfRegion {
        left: bbox.left(),
        bottom: bbox.top(),
        width: bbox.width,
        height: bbox.height,
      },
      page_left,
      page_width,
      col,
    ) {
      out.push((rows, dynamic_image));
    }
  }
  out
}

pub(crate) fn render_dynamic_image_region(
  dynamic_image: &image::DynamicImage,
  region: PdfRegion,
  page_left: f32,
  page_width: f32,
  col: usize,
) -> Option<VisualImageRows> {
  let left_cells = pdf_x_to_cells(region.left, page_left, page_width, col);
  let left_cells = left_cells.min(col.saturating_sub(1));
  let width_cells = pdf_width_to_cells(region.width, page_width, col);
  let width_cells = width_cells.max(1).min(col.saturating_sub(left_cells));
  if width_cells == 0 {
    return None;
  }
  let height_rows =
    pdf_image_height_rows(region.width, region.height, width_cells);
  let lines = render_half_block(
    dynamic_image,
    RenderConfig::new(Some(width_cells as u32), Some(height_rows as u32)),
  );
  if lines.is_empty() {
    return None;
  }
  Some(VisualImageRows {
    top: region.top(),
    left_cells,
    width_cells,
    region,
    lines,
  })
}
