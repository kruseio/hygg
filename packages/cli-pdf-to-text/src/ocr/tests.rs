#[cfg(feature = "ocr")]
fn region(left: f32, bottom: f32, right: f32, top: f32) -> super::TextRegion {
  super::TextRegion { left, bottom, right, top }
}

#[cfg(feature = "ocr")]
fn positioned_text(
  text: &str,
  region: super::TextRegion,
) -> super::PositionedText {
  super::PositionedText { text: text.to_string(), region, confidence: 1.0 }
}

#[test]
#[cfg(not(feature = "ocr"))]
fn no_feature_ocr_returns_actionable_error() {
  let err = super::pdf_to_text_with_bundled_ocr("unused.pdf")
    .expect_err("OCR should be unavailable without the bundled feature");
  assert!(err.to_string().contains("--features ocr"));
}

// Fetches the models from the `ocr-models-v1.0` release on first run (or reads
// a pre-seeded `HYGG_OCR_MODEL_DIR`), then caches them — so this needs either
// network or a warm cache, unlike the old include_bytes! assets.
#[test]
#[cfg(feature = "ocr")]
fn bundled_ocr_engine_loads_fetched_models() {
  super::bundled_ocr_engine().expect("OCR model assets should initialize");
}

#[test]
#[cfg(feature = "ocr")]
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
#[cfg(feature = "ocr")]
fn hybrid_merge_uses_ocr_when_native_text_is_empty() {
  let ocr = vec![positioned_text("Scan Text", region(10.0, 10.0, 100.0, 30.0))];
  assert_eq!(
    super::merge_native_and_ocr_regions_text("", &[], &ocr),
    "Scan Text"
  );
}

#[test]
#[cfg(feature = "ocr")]
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
#[cfg(feature = "ocr")]
fn hybrid_merge_deduplicates_case_and_punctuation_variants() {
  let native_region = region(10.0, 10.0, 140.0, 30.0);
  let native =
    vec![positioned_text("Figure 2-1: Version control", native_region.clone())];
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
#[cfg(feature = "ocr")]
fn hybrid_merge_keeps_same_text_when_position_is_not_nearby() {
  let native =
    vec![positioned_text("Status OK", region(10.0, 80.0, 100.0, 100.0))];
  let ocr = vec![positioned_text("status ok", region(10.0, 10.0, 100.0, 30.0))];
  assert_eq!(
    super::merge_native_and_ocr_regions_text("Status OK", &native, &ocr),
    "Status OK\nstatus ok"
  );
}

#[test]
#[cfg(feature = "ocr")]
fn bundled_ocr_reads_generated_image_with_confidence() {
  let engine =
    super::bundled_ocr_engine().expect("OCR engine should initialize");
  let image = generated_ocr_fixture("HELLO OCR");
  let output =
    engine.ocr_image(&image).expect("generated image should OCR successfully");
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

#[cfg(feature = "ocr")]
fn generated_ocr_fixture(text: &str) -> image::DynamicImage {
  let scale = 12u32;
  let glyph_width = 5u32;
  let glyph_height = 7u32;
  let spacing = 2u32;
  let padding = 24u32;
  let width =
    padding * 2 + text.chars().count() as u32 * (glyph_width + spacing) * scale;
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

#[cfg(feature = "ocr")]
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

#[cfg(feature = "ocr")]
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
