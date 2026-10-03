mod engine;
mod extract;
// Runtime download + verify + cache of the OCR ONNX models. Only compiled with
// the `ocr` feature (it pulls ureq/sha2/dirs); the models are no longer
// embedded.
#[cfg(feature = "ocr")]
mod files;
mod merge;
mod region;
#[cfg(feature = "ocr")]
mod session;

#[cfg(test)]
mod tests;

pub(crate) use engine::pdf_to_text_with_bundled_ocr;

#[cfg(feature = "ocr")]
pub(crate) use engine::{
  bundled_ocr_engine, bundled_ocr_engine_with, ocr_size_guarded,
};
#[cfg(feature = "ocr")]
pub(crate) use merge::{merge_native_and_ocr_regions_text, normalized_text};
#[cfg(feature = "ocr")]
pub(crate) use region::{PositionedText, TextRegion};
#[cfg(feature = "ocr")]
pub use session::{
  OCR_MODELS_DOWNLOAD_BYTES, OcrLine, OcrSession, ocr_models_cached,
};
