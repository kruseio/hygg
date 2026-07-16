#[cfg(feature = "ocr")]
use super::extract::{extract_native_text_regions, ocr_missing_text_regions};
#[cfg(feature = "ocr")]
use super::merge::merge_native_and_ocr_regions_text;

#[cfg(feature = "ocr")]
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
#[cfg(feature = "ocr")]
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

/// Build the OCR engine, downloading the ONNX models on first use.
///
/// The detection/recognition models and the dictionary are fetched from this
/// project's `ocr-models-v1.0` GitHub release, verified against pinned digests,
/// and cached under the platform cache dir (see [`super::files`]). After the
/// first call they load straight from disk. The bytes are handed to tract,
/// which parses and executes them, so only a sha256-verified copy is ever read
/// back.
#[cfg(feature = "ocr")]
pub(crate) fn bundled_ocr_engine()
-> Result<pdf_oxide::ocr::OcrEngine, Box<dyn std::error::Error>> {
  let (det_path, rec_path, dict_path) = super::files::ensure_ocr_models()?;
  let det_model = std::fs::read(&det_path)?;
  let rec_model = std::fs::read(&rec_path)?;
  let dict = std::fs::read_to_string(&dict_path)?;
  pdf_oxide::ocr::OcrEngine::from_bytes(
    &det_model,
    &rec_model,
    &dict,
    bundled_ocr_config(),
  )
  .map_err(|e| format!("failed to initialize OCR engine: {e}").into())
}

#[cfg(feature = "ocr")]
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

#[cfg(not(feature = "ocr"))]
pub(crate) fn pdf_to_text_with_bundled_ocr(
  _pdf_path: &str,
) -> Result<String, Box<dyn std::error::Error>> {
  Err(
    "OCR support is not available in this build. Rebuild with `--features ocr` to use the bundled English OCR engine."
      .into(),
  )
}
