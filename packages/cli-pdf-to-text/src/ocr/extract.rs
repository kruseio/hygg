#[cfg(feature = "ocr")]
use super::engine::ocr_size_guarded;
#[cfg(feature = "ocr")]
use super::merge::normalized_text;
#[cfg(feature = "ocr")]
use super::region::{PositionedText, TextRegion};

#[cfg(feature = "ocr")]
pub(crate) fn extract_native_text_regions(
  doc: &pdf_oxide::PdfDocument,
  page: usize,
) -> Vec<PositionedText> {
  let mut out = Vec::new();
  // catch_unwind for the same reason every other pdf_oxide extraction call in
  // this workspace has one (stream/core.rs, stream/vector.rs, visuals.rs): a
  // page that makes the extractor panic should cost that page, not the process.
  // A bare `let Ok(..) else` handles the Err it returns and nothing about the
  // panic it may raise instead, and `hygg --ocr` walks every page of a file the
  // user merely opened.
  let lines = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    doc.extract_text_lines(page)
  }))
  .ok()
  .and_then(Result::ok)
  .unwrap_or_default();

  for line in lines {
    let text = line
      .words
      .iter()
      .map(|word| word.text.as_str())
      .collect::<Vec<_>>()
      .join(" ");
    let text = crate::sanitize::sanitize_layout_text(&text);
    if text.trim().is_empty() {
      continue;
    }
    let Some(region) = TextRegion::from_rect(&line.bbox) else {
      continue;
    };
    out.push(PositionedText { text, region, confidence: 1.0 });
  }

  out
}

#[cfg(feature = "ocr")]
pub(crate) fn ocr_missing_text_regions(
  doc: &pdf_oxide::PdfDocument,
  page: usize,
  engine: &pdf_oxide::ocr::OcrEngine,
  native_regions: &[PositionedText],
) -> Vec<PositionedText> {
  let mut out = Vec::new();

  let images = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    doc.extract_images(page)
  }))
  .ok()
  .and_then(Result::ok)
  .unwrap_or_default();
  for image in images {
    let Some(bbox) = image.bbox() else {
      continue;
    };
    let Some(region) = TextRegion::from_rect(bbox) else {
      continue;
    };
    if native_region_text_is_sufficient(native_regions, &region) {
      continue;
    }
    let Ok(dynamic_image) = image.to_dynamic_image() else {
      continue;
    };
    out.extend(ocr_dynamic_image_region(engine, &dynamic_image, &region));
  }

  for region in detect_vector_diagram_regions(doc, page) {
    if native_region_text_is_sufficient(native_regions, &region) {
      continue;
    }
    // The region is clamped to the media box and nothing else, and the media
    // box is a number the document chose. See stream/vector.rs for the same
    // guard on the sibling path: 120 DPI turns the 14400pt page the spec allows
    // into a 24000x24000 raster, 2.3 GB, before anything looks at it.
    if region_raster_is_oversized(region.width(), region.height()) {
      continue;
    }
    let options = pdf_oxide::rendering::RenderOptions::with_dpi(120);
    let rendered = pdf_oxide::rendering::render_page_region(
      doc,
      page,
      (region.left, region.bottom, region.width(), region.height()),
      &options,
    );
    let Ok(rendered) = rendered else {
      continue;
    };
    let Ok(dynamic_image) = image::load_from_memory(&rendered.data) else {
      continue;
    };
    out.extend(ocr_dynamic_image_region(engine, &dynamic_image, &region));
  }

  super::merge::dedupe_positioned_ocr(out)
}

/// 64 megapixels of 120-DPI raster (~256 MB as RGBA). A letter page is 2.6 MP,
/// and a region is a part of one; anything past this is not a diagram.
#[cfg(feature = "ocr")]
fn region_raster_is_oversized(width: f32, height: f32) -> bool {
  const MAX_REGION_PIXELS: f32 = 64_000_000.0;
  const SCALE: f32 = 120.0 / 72.0;

  let pixels = (width * SCALE) * (height * SCALE);
  !pixels.is_finite() || pixels > MAX_REGION_PIXELS
}

#[cfg(feature = "ocr")]
fn ocr_dynamic_image_region(
  engine: &pdf_oxide::ocr::OcrEngine,
  image: &image::DynamicImage,
  pdf_region: &TextRegion,
) -> Vec<PositionedText> {
  let image = ocr_size_guarded(image);
  let image = image.as_ref();
  let Ok(ocr) = engine.ocr_image(image) else {
    return Vec::new();
  };
  let image_width = image.width().max(1) as f32;
  let image_height = image.height().max(1) as f32;

  let mut out = Vec::new();
  for span in ocr.spans {
    let text = crate::sanitize::sanitize_layout_text(span.text.trim());
    if text.trim().is_empty() {
      continue;
    }
    let Some(region) = ocr_polygon_to_pdf_region(
      &span.polygon,
      pdf_region,
      image_width,
      image_height,
    ) else {
      continue;
    };
    out.push(PositionedText { text, region, confidence: span.confidence });
  }
  out
}

#[cfg(feature = "ocr")]
fn ocr_polygon_to_pdf_region(
  polygon: &[[f32; 2]; 4],
  pdf_region: &TextRegion,
  image_width: f32,
  image_height: f32,
) -> Option<TextRegion> {
  let mut min_x = f32::INFINITY;
  let mut max_x = 0.0_f32;
  let mut min_y = f32::INFINITY;
  let mut max_y = 0.0_f32;

  for [x, y] in polygon {
    if !x.is_finite() || !y.is_finite() {
      return None;
    }
    min_x = min_x.min(*x);
    max_x = max_x.max(*x);
    min_y = min_y.min(*y);
    max_y = max_y.max(*y);
  }
  if max_x <= min_x || max_y <= min_y {
    return None;
  }

  let left = pdf_region.left + (min_x / image_width) * pdf_region.width();
  let right = pdf_region.left + (max_x / image_width) * pdf_region.width();
  let top = pdf_region.top - (min_y / image_height) * pdf_region.height();
  let bottom = pdf_region.top - (max_y / image_height) * pdf_region.height();
  if right <= left || top <= bottom {
    return None;
  }

  Some(TextRegion { left, bottom, right, top })
}

#[cfg(feature = "ocr")]
fn native_region_text_is_sufficient(
  native_regions: &[PositionedText],
  region: &TextRegion,
) -> bool {
  let native_text = native_regions
    .iter()
    .filter(|native| native.region.overlaps_or_near(region))
    .map(|native| native.text.as_str())
    .collect::<Vec<_>>()
    .join(" ");
  normalized_text(&native_text).len() >= 8
}

#[cfg(feature = "ocr")]
fn detect_vector_diagram_regions(
  doc: &pdf_oxide::PdfDocument,
  page: usize,
) -> Vec<TextRegion> {
  let Ok((llx, lly, urx, ury)) = doc.get_page_media_box(page) else {
    return Vec::new();
  };
  let page_left = llx.min(urx);
  let page_top = lly.min(ury);
  let page_width = (urx - llx).abs();
  let page_height = (ury - lly).abs();
  if page_width <= 0.0 || page_height <= 0.0 {
    return Vec::new();
  }
  let page_right = page_left + page_width;
  let page_bottom = page_top + page_height;
  let paths = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    doc.extract_paths(page)
  }))
  .ok()
  .and_then(Result::ok)
  .unwrap_or_default();

  let mut count = 0usize;
  let mut left = f32::INFINITY;
  let mut bottom = f32::INFINITY;
  let mut right = f32::NEG_INFINITY;
  let mut top = f32::NEG_INFINITY;

  for path in paths {
    let bbox = path.bbox;
    if !path.is_table_primitive()
      || !bbox.x.is_finite()
      || !bbox.y.is_finite()
      || !bbox.width.is_finite()
      || !bbox.height.is_finite()
      || (bbox.width <= 0.0 && bbox.height <= 0.0)
      || bbox.width > page_width * 0.95
      || bbox.height > page_height * 0.95
    {
      continue;
    }

    count += 1;
    left = left.min(bbox.left());
    bottom = bottom.min(bbox.top());
    right = right.max(bbox.right());
    top = top.max(bbox.bottom());
  }

  if count < 3 || !left.is_finite() || !bottom.is_finite() {
    return Vec::new();
  }

  let pad = 4.0;
  let padded_left = (left - pad).max(page_left);
  let padded_bottom = (bottom - pad).max(page_top);
  let padded_right = (right + pad).min(page_right);
  let padded_top = (top + pad).min(page_bottom);
  let region = TextRegion {
    left: padded_left,
    bottom: padded_bottom,
    right: padded_right,
    top: padded_top,
  };

  if region.width() < 24.0 || region.height() < 24.0 {
    Vec::new()
  } else {
    vec![region]
  }
}
