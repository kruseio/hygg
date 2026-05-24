#[cfg(feature = "pdf-ocr-bundled")]
use std::io::Read;

#[cfg(feature = "pdf-ocr-bundled")]
use flate2::read::GzDecoder;

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
pub(crate) fn bundled_ocr_engine(
) -> Result<pdf_oxide::ocr::OcrEngine, Box<dyn std::error::Error>> {
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
  let options = pdf_oxide::ocr::OcrExtractOptions {
    config: bundled_ocr_config(),
    fallback_to_native: true,
    ..Default::default()
  };

  let mut pages = Vec::with_capacity(page_count);
  for page in 0..page_count {
    let native = doc
      .extract_text(page)
      .ok()
      .map(|text| crate::sanitize::sanitize_layout_text(&text))
      .unwrap_or_default();
    let page_type = pdf_oxide::ocr::detect_page_type(&doc, page)
      .unwrap_or(pdf_oxide::ocr::PageType::NativeText);

    let page_text = match page_type {
      pdf_oxide::ocr::PageType::NativeText => native,
      pdf_oxide::ocr::PageType::ScannedPage => {
        let ocr = pdf_oxide::ocr::ocr_page(&doc, page, &engine, &options)
          .unwrap_or_default();
        if ocr.trim().is_empty() { native } else { ocr }
      }
      pdf_oxide::ocr::PageType::HybridPage => {
        let ocr = pdf_oxide::ocr::ocr_page(&doc, page, &engine, &options)
          .unwrap_or_default();
        merge_native_and_ocr_text(&native, &ocr)
      }
    };

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
fn merge_native_and_ocr_text(native: &str, ocr: &str) -> String {
  let native = native.trim();
  let ocr = ocr.trim();
  if native.is_empty() {
    return ocr.to_string();
  }
  if ocr.is_empty() || normalized_text(native).contains(&normalized_text(ocr)) {
    return native.to_string();
  }
  format!("{native}\n{ocr}")
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
    assert_eq!(
      super::merge_native_and_ocr_text("Hello World", "hello world"),
      "Hello World"
    );
  }
}
