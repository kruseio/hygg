#![cfg(feature = "pdf-ocr-bundled")]

use super::*;

#[test]
fn ocr_text_rows_overlay_existing_ansi_art() {
  let engine =
    crate::ocr::bundled_ocr_engine().expect("bundled OCR should initialize");
  let image = generated_ocr_fixture("HELLO OCR");
  let text_rows = ocr_dynamic_image_text_rows(
    &engine,
    &image,
    PdfRegion { left: 0.0, bottom: 100.0, width: 300.0, height: 80.0 },
  );
  assert!(
    text_rows.iter().any(|row| {
      let normalized = normalized_visual_text(&row.text);
      normalized.contains("hello") || normalized.contains("ocr")
    }),
    "OCR should produce overlayable text rows, got {:?}",
    text_rows
  );
  let image_rows = vec![VisualImageRows {
    top: 180.0,
    left_cells: 0,
    width_cells: 60,
    region: PdfRegion { left: 0.0, bottom: 100.0, width: 300.0, height: 80.0 },
    lines: (0..6)
      .map(|_| format!("\x1b[38;2;1;2;3m{}\x1b[0m", "▀".repeat(60)))
      .collect(),
  }];

  // Overlay the OCR rows onto the existing ANSI art (no standalone text
  // emission), which is the scenario this test exercises: `compose_visual_page`
  // instead emits the rows as separate `Text` lines (its purpose in the
  // `tests_compose` cases), which is not an overlay.
  let page =
    compose_visual_page_with_overlay(Vec::new(), text_rows, image_rows, 80);
  let rendered = page.lines.join("\n");
  let normalized = normalized_visual_text(&rendered);

  assert!(page.line_kinds.iter().all(|kind| *kind == PdfLineKind::AnsiArt));
  assert!(
    normalized.contains("hello") || normalized.contains("ocr"),
    "OCR text should be overlaid into ANSI art, got {rendered:?}"
  );
}

#[test]
fn ocrs_images_when_page_has_no_native_text() {
  let region =
    PdfRegion { left: 0.0, bottom: 0.0, width: 100.0, height: 100.0 };

  assert!(should_ocr_image_region(region, &[]));
}

#[test]
fn ocrs_captioned_images_without_native_text() {
  let region =
    PdfRegion { left: 48.0, bottom: 300.0, width: 500.0, height: 200.0 };
  let native_rows = vec![
    VisualTextRow {
      top: 285.0,
      left: 48.0,
      text: "Figure 8. The lifecycle of the status of your files".to_string(),
    },
    VisualTextRow {
      top: 250.0,
      left: 48.0,
      text: "Checking the Status of Your Files".to_string(),
    },
  ];

  assert!(should_ocr_image_region(region, &native_rows));
}

#[test]
fn skips_uncaptioned_images_on_native_text_pages() {
  let region =
    PdfRegion { left: 48.0, bottom: 300.0, width: 500.0, height: 200.0 };
  let native_rows = vec![VisualTextRow {
    top: 250.0,
    left: 48.0,
    text: "Body text below an unrelated decorative image".to_string(),
  }];

  assert!(!should_ocr_image_region(region, &native_rows));
}

#[test]
fn skips_ocr_when_native_text_already_covers_region() {
  let region =
    PdfRegion { left: 48.0, bottom: 300.0, width: 500.0, height: 200.0 };
  let native_rows = vec![
    VisualTextRow { top: 400.0, left: 100.0, text: "Native label".to_string() },
    VisualTextRow {
      top: 285.0,
      left: 48.0,
      text: "Figure 1. Native diagram".to_string(),
    },
  ];

  assert!(!should_ocr_image_region(region, &native_rows));
}

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
