use crate::stream::text_rows::normalize_visual_text_row;
use crate::stream::types::{PdfRegion, VisualTextRow};
use crate::stream::vector::page_metrics;
use crate::stream::vector_detect::detect_vector_diagram_regions;
use crate::stream::vector_geom::{
  has_nearby_figure_caption, visual_text_row_overlaps_region,
};

pub(crate) fn ocr_visual_text_rows(
  doc: &pdf_oxide::PdfDocument,
  page_0based: usize,
  images: &[pdf_oxide::extractors::PdfImage],
  engine: &pdf_oxide::ocr::OcrEngine,
  native_rows: &[VisualTextRow],
) -> Vec<VisualTextRow> {
  let mut out = Vec::new();
  for image in images {
    let Some(bbox) = image.bbox() else {
      continue;
    };
    if bbox.width <= 0.0 || bbox.height <= 0.0 {
      continue;
    }
    let region = PdfRegion {
      left: bbox.left(),
      bottom: bbox.top(),
      width: bbox.width,
      height: bbox.height,
    };
    if !should_ocr_image_region(region, native_rows) {
      continue;
    }
    let Ok(dynamic_image) = image.to_dynamic_image() else {
      continue;
    };
    out.extend(ocr_dynamic_image_text_rows(engine, &dynamic_image, region));
  }

  for (region, dynamic_image) in
    render_vector_diagram_images(doc, page_0based, native_rows)
  {
    if !should_ocr_image_region(region, native_rows) {
      continue;
    }
    out.extend(ocr_dynamic_image_text_rows(engine, &dynamic_image, region));
  }

  out
}

pub(crate) fn should_ocr_image_region(
  region: PdfRegion,
  native_rows: &[VisualTextRow],
) -> bool {
  if native_text_is_sufficient_in_region(native_rows, region) {
    return false;
  }
  if native_rows.is_empty() {
    return true;
  }
  has_nearby_figure_caption(region, native_rows)
}

fn native_text_is_sufficient_in_region(
  native_rows: &[VisualTextRow],
  region: PdfRegion,
) -> bool {
  let text = native_rows
    .iter()
    .filter(|row| visual_text_row_overlaps_region(row, region))
    .map(|row| row.text.as_str())
    .collect::<Vec<_>>()
    .join(" ");
  normalized_visual_text(&text).len() >= 8
}

pub(crate) fn ocr_dynamic_image_text_rows(
  engine: &pdf_oxide::ocr::OcrEngine,
  image: &image::DynamicImage,
  pdf_region: PdfRegion,
) -> Vec<VisualTextRow> {
  let Ok(output) = engine.ocr_image(image) else {
    return Vec::new();
  };
  let image_width = image.width().max(1) as f32;
  let image_height = image.height().max(1) as f32;

  output
    .spans
    .into_iter()
    .filter_map(|span| {
      let text = normalize_visual_text_row(span.text.trim());
      if text.trim().is_empty() {
        return None;
      }
      let (left, top) = ocr_polygon_pdf_anchor(
        &span.polygon,
        pdf_region,
        image_width,
        image_height,
      )?;
      Some(VisualTextRow { top, left, text })
    })
    .collect()
}

fn ocr_polygon_pdf_anchor(
  polygon: &[[f32; 2]; 4],
  pdf_region: PdfRegion,
  image_width: f32,
  image_height: f32,
) -> Option<(f32, f32)> {
  let mut min_x = f32::INFINITY;
  let mut min_y = f32::INFINITY;
  for [x, y] in polygon {
    if !x.is_finite() || !y.is_finite() {
      return None;
    }
    min_x = min_x.min(*x);
    min_y = min_y.min(*y);
  }
  if !min_x.is_finite() || !min_y.is_finite() {
    return None;
  }
  let left = pdf_region.left + (min_x / image_width) * pdf_region.width;
  let top = pdf_region.top() - (min_y / image_height) * pdf_region.height;
  Some((left, top))
}

fn render_vector_diagram_images(
  doc: &pdf_oxide::PdfDocument,
  page_0based: usize,
  native_rows: &[VisualTextRow],
) -> Vec<(PdfRegion, image::DynamicImage)> {
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
    true,
  );
  let options = pdf_oxide::rendering::RenderOptions::with_dpi(120);

  regions
    .into_iter()
    .filter(|region| should_ocr_image_region(*region, native_rows))
    .filter_map(|region| {
      let rendered = pdf_oxide::rendering::render_page_region(
        doc,
        page_0based,
        (region.left, region.bottom, region.width, region.height),
        &options,
      )
      .ok()?;
      let dynamic_image = image::load_from_memory(&rendered.data).ok()?;
      Some((region, dynamic_image))
    })
    .collect()
}

pub(crate) fn has_near_duplicate_visual_text(
  native_rows: &[VisualTextRow],
  ocr_row: &VisualTextRow,
) -> bool {
  let ocr_norm = normalized_visual_text(&ocr_row.text);
  if ocr_norm.is_empty() {
    return true;
  }
  native_rows.iter().any(|native| {
    (native.top - ocr_row.top).abs() <= 12.0
      && (native.left - ocr_row.left).abs() <= 24.0
      && {
        let native_norm = normalized_visual_text(&native.text);
        native_norm.contains(&ocr_norm) || ocr_norm.contains(&native_norm)
      }
  })
}

pub(crate) fn normalized_visual_text(text: &str) -> String {
  text
    .chars()
    .filter(|ch| ch.is_alphanumeric())
    .flat_map(char::to_lowercase)
    .collect()
}
