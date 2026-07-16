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

/// A ceiling on what gets handed to the OCR engine, for absurd input only.
///
/// An earlier version of this pipeline resized every OCR input down to a 240px
/// longest edge. That was removed deliberately — 240px is below what the
/// recognizer can read, and it cost real accuracy on real pages — but it was
/// also the only thing bounding the work an embedded image could ask for. The
/// image comes out of the document: `det_max_side(960)` above downscales for
/// the detection pass, while the recognition crops still come off whatever was
/// passed in, so a page carrying one 40000x40000 image sets the CPU going for
/// as long as it likes.
///
/// 4000px is over four times the detector's own working size, so no page a
/// person would try to read is touched, and `resize` preserves the aspect
/// ratio, so callers normalizing OCR anchors against the returned image's
/// dimensions stay correct. Borrowed, not copied, in the case that matters.
#[cfg(feature = "pdf-ocr-bundled")]
pub(crate) fn ocr_size_guarded(
  image: &image::DynamicImage,
) -> std::borrow::Cow<'_, image::DynamicImage> {
  const MAX_OCR_IMAGE_EDGE: u32 = 4000;

  if image.width() <= MAX_OCR_IMAGE_EDGE && image.height() <= MAX_OCR_IMAGE_EDGE
  {
    return std::borrow::Cow::Borrowed(image);
  }
  std::borrow::Cow::Owned(image.resize(
    MAX_OCR_IMAGE_EDGE,
    MAX_OCR_IMAGE_EDGE,
    image::imageops::FilterType::Triangle,
  ))
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
