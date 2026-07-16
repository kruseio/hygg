mod engine;
mod extract;
// Runtime download + verify + cache of the OCR ONNX models. Only compiled with
// the `ocr` feature (it pulls ureq/sha2/dirs); the models are no longer
// embedded.
#[cfg(feature = "ocr")]
mod files;
mod merge;
mod region;

#[cfg(test)]
mod tests;

pub(crate) use engine::pdf_to_text_with_bundled_ocr;

#[cfg(feature = "ocr")]
pub(crate) use engine::{bundled_ocr_engine, ocr_size_guarded};
#[cfg(feature = "ocr")]
pub(crate) use merge::{merge_native_and_ocr_regions_text, normalized_text};
#[cfg(feature = "ocr")]
pub(crate) use region::{PositionedText, TextRegion};
