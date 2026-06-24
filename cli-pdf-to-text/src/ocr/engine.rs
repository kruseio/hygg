#[cfg(feature = "pdf-ocr-bundled")]
use std::io::Read;

#[cfg(feature = "pdf-ocr-bundled")]
use flate2::read::GzDecoder;

#[cfg(feature = "pdf-ocr-bundled")]
use super::extract::{extract_native_text_regions, ocr_missing_text_regions};
#[cfg(feature = "pdf-ocr-bundled")]
use super::merge::merge_native_and_ocr_regions_text;

#[cfg(feature = "pdf-ocr-bundled")]
const DET_MODEL_GZ: &[u8] =
  include_bytes!("../../assets/ocr/monkt-paddleocr-onnx/det.onnx.gz");
#[cfg(feature = "pdf-ocr-bundled")]
const REC_MODEL_GZ: &[u8] =
  include_bytes!("../../assets/ocr/monkt-paddleocr-onnx/rec.onnx.gz");
#[cfg(feature = "pdf-ocr-bundled")]
const DICT: &str =
  include_str!("../../assets/ocr/monkt-paddleocr-onnx/dict.txt");

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
