#[cfg(feature = "pdf-ocr-bundled")]
use std::io::Read;

#[cfg(feature = "pdf-ocr-bundled")]
use flate2::read::GzDecoder;

#[cfg(feature = "pdf-ocr-bundled")]
use pdf_oxide::geometry::Rect;

#[cfg(feature = "pdf-ocr-bundled")]
const DET_MODEL_GZ: &[u8] =
  include_bytes!("../assets/ocr/monkt-paddleocr-onnx/det.onnx.gz");
#[cfg(feature = "pdf-ocr-bundled")]
const REC_MODEL_GZ: &[u8] =
  include_bytes!("../assets/ocr/monkt-paddleocr-onnx/rec.onnx.gz");
#[cfg(feature = "pdf-ocr-bundled")]
const DICT: &str = include_str!("../assets/ocr/monkt-paddleocr-onnx/dict.txt");

#[cfg(feature = "pdf-ocr-bundled")]
fn decompress_gzip(
  bytes: &[u8],
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
  let mut decoder = GzDecoder::new(bytes);
  let mut out = Vec::new();
  decoder.read_to_end(&mut out)?;
  Ok(out)
}

#[cfg(feature = "pdf-ocr-bundled")]
fn bundled_ocr_config() -> pdf_oxide::ocr::OcrConfig {
  pdf_oxide::ocr::OcrConfig::builder()
    .det_max_side(960)
    .rec_target_height(32)
    .build()
}

#[cfg(feature = "pdf-ocr-bundled")]
pub(crate) fn bundled_ocr_engine()
-> Result<pdf_oxide::ocr::OcrEngine, Box<dyn std::error::Error>> {
  let det_model = decompress_gzip(DET_MODEL_GZ)?;
  let rec_model = decompress_gzip(REC_MODEL_GZ)?;
  pdf_oxide::ocr::OcrEngine::from_bytes(
    &det_model,
    &rec_model,
    DICT,
    bundled_ocr_config(),
  )
  .map_err(|e| format!("failed to initialize bundled OCR engine: {e}").into())
}

#[cfg(feature = "pdf-ocr-bundled")]
pub(crate) fn pdf_to_text_with_bundled_ocr(
  pdf_path: &str,
) -> Result<String, Box<dyn std::error::Error>> {
  let canonical_path = hygg_shared::normalize_file_path(pdf_path)?;
  let doc = pdf_oxide::PdfDocument::open(&canonical_path)
    .map_err(|e| format!("pdf_oxide open failed: {e:?}"))?;
  let page_count = doc
    .page_count()
    .map_err(|e| format!("pdf_oxide page_count failed: {e:?}"))?;
  let engine = bundled_ocr_engine()?;

  let mut pages = Vec::with_capacity(page_count);
  for page in 0..page_count {
    let native = doc
      .extract_text(page)
      .ok()
      .map(|text| crate::sanitize::sanitize_layout_text(&text))
      .unwrap_or_default();
    let native_regions = extract_native_text_regions(&doc, page);
    let ocr_regions =
      ocr_missing_text_regions(&doc, page, &engine, &native_regions);
    let page_text =
      merge_native_and_ocr_regions_text(&native, &native_regions, &ocr_regions);

    if !page_text.trim().is_empty() {
      pages.push(page_text.trim_end().to_string());
    }
  }

  Ok(pages.join("\n\n"))
}

#[cfg(not(feature = "pdf-ocr-bundled"))]
pub(crate) fn pdf_to_text_with_bundled_ocr(
  _pdf_path: &str,
) -> Result<String, Box<dyn std::error::Error>> {
  Err(
    "OCR support is not available in this build. Rebuild with `--features pdf-ocr-bundled` to use the bundled English OCR engine."
      .into(),
  )
}

#[cfg(feature = "pdf-ocr-bundled")]
#[derive(Clone, Debug)]
struct TextRegion {
  left: f32,
  bottom: f32,
  right: f32,
  top: f32,
}

#[cfg(feature = "pdf-ocr-bundled")]
impl TextRegion {
  fn from_rect(rect: &Rect) -> Option<Self> {
    let left = rect.left();
    let right = rect.right();
    let bottom = rect.top();
    let top = rect.bottom();
    if !left.is_finite()
      || !right.is_finite()
      || !bottom.is_finite()
      || !top.is_finite()
      || right <= left
      || top <= bottom
    {
      return None;
    }
    Some(Self { left, bottom, right, top })
  }

  fn width(&self) -> f32 {
    self.right - self.left
  }

  fn height(&self) -> f32 {
    self.top - self.bottom
  }

  fn overlaps_or_near(&self, other: &Self) -> bool {
    let pad_x = self.width().max(other.width()).max(12.0) * 0.25;
    let pad_y = self.height().max(other.height()).max(12.0) * 0.75;
    self.left <= other.right + pad_x
      && self.right + pad_x >= other.left
      && self.bottom <= other.top + pad_y
      && self.top + pad_y >= other.bottom
  }
}

#[cfg(feature = "pdf-ocr-bundled")]
#[derive(Clone, Debug)]
struct PositionedText {
  text: String,
  region: TextRegion,
  confidence: f32,
}

#[cfg(feature = "pdf-ocr-bundled")]
fn extract_native_text_regions(
  doc: &pdf_oxide::PdfDocument,
  page: usize,
) -> Vec<PositionedText> {
  let mut out = Vec::new();
  let Ok(lines) = doc.extract_text_lines(page) else {
    return out;
  };

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

#[cfg(feature = "pdf-ocr-bundled")]
fn ocr_missing_text_regions(
  doc: &pdf_oxide::PdfDocument,
  page: usize,
  engine: &pdf_oxide::ocr::OcrEngine,
  native_regions: &[PositionedText],
) -> Vec<PositionedText> {
  let mut out = Vec::new();

  if let Ok(images) = doc.extract_images(page) {
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
  }

  for region in detect_vector_diagram_regions(doc, page) {
    if native_region_text_is_sufficient(native_regions, &region) {
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

  dedupe_positioned_ocr(out)
}

#[cfg(feature = "pdf-ocr-bundled")]
fn ocr_dynamic_image_region(
  engine: &pdf_oxide::ocr::OcrEngine,
  image: &image::DynamicImage,
  pdf_region: &TextRegion,
) -> Vec<PositionedText> {
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

#[cfg(feature = "pdf-ocr-bundled")]
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

#[cfg(feature = "pdf-ocr-bundled")]
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

#[cfg(feature = "pdf-ocr-bundled")]
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
  let Ok(paths) = doc.extract_paths(page) else {
    return Vec::new();
  };

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

#[cfg(feature = "pdf-ocr-bundled")]
fn dedupe_positioned_ocr(
  ocr_regions: Vec<PositionedText>,
) -> Vec<PositionedText> {
  let mut out: Vec<PositionedText> = Vec::new();
  for region in ocr_regions {
    let normalized = normalized_text(&region.text);
    if normalized.is_empty() {
      continue;
    }
    let mut duplicate_index = None;
    for (idx, existing) in out.iter().enumerate() {
      let existing_normalized = normalized_text(&existing.text);
      if existing.region.overlaps_or_near(&region.region)
        && (existing_normalized.contains(&normalized)
          || normalized.contains(&existing_normalized))
      {
        duplicate_index = Some(idx);
        break;
      }
    }
    if let Some(idx) = duplicate_index {
      if region.confidence > out[idx].confidence {
        out[idx] = region;
      }
      continue;
    }
    out.push(region);
  }
  out
}

#[cfg(feature = "pdf-ocr-bundled")]
fn merge_native_and_ocr_regions_text(
  native: &str,
  native_regions: &[PositionedText],
  ocr_regions: &[PositionedText],
) -> String {
  let native = native.trim();
  let mut extra = Vec::new();
  for ocr in ocr_regions {
    if is_native_duplicate(native_regions, ocr) {
      continue;
    }
    extra.push(ocr);
  }

  extra.sort_by(|a, b| {
    b.region
      .top
      .partial_cmp(&a.region.top)
      .unwrap_or(std::cmp::Ordering::Equal)
      .then_with(|| {
        a.region
          .left
          .partial_cmp(&b.region.left)
          .unwrap_or(std::cmp::Ordering::Equal)
      })
  });

  if extra.is_empty() {
    return native.to_string();
  }
  let ocr = extra
    .iter()
    .map(|region| region.text.trim())
    .filter(|text| !text.is_empty())
    .collect::<Vec<_>>()
    .join("\n");
  if native.is_empty() { ocr } else { format!("{native}\n{ocr}") }
}

#[cfg(feature = "pdf-ocr-bundled")]
fn is_native_duplicate(
  native_regions: &[PositionedText],
  ocr: &PositionedText,
) -> bool {
  let ocr_normalized = normalized_text(&ocr.text);
  if ocr_normalized.is_empty() {
    return true;
  }
  native_regions.iter().any(|native| {
    let native_normalized = normalized_text(&native.text);
    native.region.overlaps_or_near(&ocr.region)
      && (native_normalized.contains(&ocr_normalized)
        || ocr_normalized.contains(&native_normalized))
  })
}

#[cfg(feature = "pdf-ocr-bundled")]
fn normalized_text(text: &str) -> String {
  text
    .chars()
    .filter(|ch| ch.is_alphanumeric())
    .flat_map(char::to_lowercase)
    .collect()
}

#[cfg(test)]
mod tests {
  #[cfg(feature = "pdf-ocr-bundled")]
  fn region(left: f32, bottom: f32, right: f32, top: f32) -> super::TextRegion {
    super::TextRegion { left, bottom, right, top }
  }

  #[cfg(feature = "pdf-ocr-bundled")]
  fn positioned_text(
    text: &str,
    region: super::TextRegion,
  ) -> super::PositionedText {
    super::PositionedText { text: text.to_string(), region, confidence: 1.0 }
  }

  #[test]
  #[cfg(not(feature = "pdf-ocr-bundled"))]
  fn no_feature_ocr_returns_actionable_error() {
    let err = super::pdf_to_text_with_bundled_ocr("unused.pdf")
      .expect_err("OCR should be unavailable without the bundled feature");
    assert!(err.to_string().contains("--features pdf-ocr-bundled"));
  }

  #[test]
  #[cfg(feature = "pdf-ocr-bundled")]
  fn bundled_ocr_engine_loads_embedded_assets() {
    super::bundled_ocr_engine()
      .expect("embedded OCR model assets should initialize");
  }

  #[test]
  #[cfg(feature = "pdf-ocr-bundled")]
  fn hybrid_merge_prefers_native_duplicate_text() {
    let native_region = region(10.0, 10.0, 100.0, 30.0);
    let native = vec![positioned_text("Hello World", native_region.clone())];
    let ocr = vec![positioned_text("hello world", native_region)];
    assert_eq!(
      super::merge_native_and_ocr_regions_text("Hello World", &native, &ocr),
      "Hello World"
    );
  }

  #[test]
  #[cfg(feature = "pdf-ocr-bundled")]
  fn hybrid_merge_uses_ocr_when_native_text_is_empty() {
    let ocr =
      vec![positioned_text("Scan Text", region(10.0, 10.0, 100.0, 30.0))];
    assert_eq!(
      super::merge_native_and_ocr_regions_text("", &[], &ocr),
      "Scan Text"
    );
  }

  #[test]
  #[cfg(feature = "pdf-ocr-bundled")]
  fn hybrid_merge_appends_distinct_ocr_text() {
    let native =
      vec![positioned_text("Native label", region(10.0, 60.0, 100.0, 80.0))];
    let ocr =
      vec![positioned_text("Scanned label", region(10.0, 10.0, 100.0, 30.0))];
    assert_eq!(
      super::merge_native_and_ocr_regions_text("Native label", &native, &ocr),
      "Native label\nScanned label"
    );
  }

  #[test]
  #[cfg(feature = "pdf-ocr-bundled")]
  fn hybrid_merge_deduplicates_case_and_punctuation_variants() {
    let native_region = region(10.0, 10.0, 140.0, 30.0);
    let native = vec![positioned_text(
      "Figure 2-1: Version control",
      native_region.clone(),
    )];
    let ocr = vec![positioned_text("figure 21 version control", native_region)];
    assert_eq!(
      super::merge_native_and_ocr_regions_text(
        "Figure 2-1: Version control",
        &native,
        &ocr,
      ),
      "Figure 2-1: Version control"
    );
  }

  #[test]
  #[cfg(feature = "pdf-ocr-bundled")]
  fn hybrid_merge_keeps_same_text_when_position_is_not_nearby() {
    let native =
      vec![positioned_text("Status OK", region(10.0, 80.0, 100.0, 100.0))];
    let ocr =
      vec![positioned_text("status ok", region(10.0, 10.0, 100.0, 30.0))];
    assert_eq!(
      super::merge_native_and_ocr_regions_text("Status OK", &native, &ocr),
      "Status OK\nstatus ok"
    );
  }

  #[test]
  #[cfg(feature = "pdf-ocr-bundled")]
  fn bundled_ocr_reads_generated_image_with_confidence() {
    let engine = super::bundled_ocr_engine()
      .expect("embedded OCR engine should initialize");
    let image = generated_ocr_fixture("HELLO OCR");
    let output = engine
      .ocr_image(&image)
      .expect("generated image should OCR successfully");
    let recognized = super::normalized_text(&output.text_in_reading_order());

    assert!(
      recognized.contains("hello") || recognized.contains("ocr"),
      "recognized text should contain expected English text, got {:?}",
      output.text_in_reading_order()
    );
    assert!(
      output.total_confidence >= 0.50,
      "OCR confidence should clear the recognizer threshold, got {}",
      output.total_confidence
    );
  }

  #[cfg(feature = "pdf-ocr-bundled")]
  fn generated_ocr_fixture(text: &str) -> image::DynamicImage {
    let scale = 12u32;
    let glyph_width = 5u32;
    let glyph_height = 7u32;
    let spacing = 2u32;
    let padding = 24u32;
    let width = padding * 2
      + text.chars().count() as u32 * (glyph_width + spacing) * scale;
    let height = padding * 2 + glyph_height * scale;
    let mut image = image::RgbaImage::from_pixel(
      width,
      height,
      image::Rgba([255, 255, 255, 255]),
    );

    let mut x = padding;
    for ch in text.chars() {
      if ch == ' ' {
        x += (glyph_width + spacing) * scale;
        continue;
      }
      draw_glyph(&mut image, x, padding, scale, ch);
      x += (glyph_width + spacing) * scale;
    }

    image::DynamicImage::ImageRgba8(image)
  }

  #[cfg(feature = "pdf-ocr-bundled")]
  fn draw_glyph(
    image: &mut image::RgbaImage,
    x: u32,
    y: u32,
    scale: u32,
    ch: char,
  ) {
    let Some(pattern) = glyph_pattern(ch) else {
      return;
    };
    for (row, bits) in pattern.iter().enumerate() {
      for (col, bit) in bits.chars().enumerate() {
        if bit != '1' {
          continue;
        }
        for dy in 0..scale {
          for dx in 0..scale {
            image.put_pixel(
              x + col as u32 * scale + dx,
              y + row as u32 * scale + dy,
              image::Rgba([0, 0, 0, 255]),
            );
          }
        }
      }
    }
  }

  #[cfg(feature = "pdf-ocr-bundled")]
  fn glyph_pattern(ch: char) -> Option<[&'static str; 7]> {
    match ch {
      'C' => {
        Some(["01111", "10000", "10000", "10000", "10000", "10000", "01111"])
      }
      'E' => {
        Some(["11111", "10000", "10000", "11110", "10000", "10000", "11111"])
      }
      'H' => {
        Some(["10001", "10001", "10001", "11111", "10001", "10001", "10001"])
      }
      'L' => {
        Some(["10000", "10000", "10000", "10000", "10000", "10000", "11111"])
      }
      'O' => {
        Some(["01110", "10001", "10001", "10001", "10001", "10001", "01110"])
      }
      'R' => {
        Some(["11110", "10001", "10001", "11110", "10100", "10010", "10001"])
      }
      _ => None,
    }
  }
}
