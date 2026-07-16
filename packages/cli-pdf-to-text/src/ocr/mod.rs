mod engine;
mod extract;
mod merge;
mod region;

#[cfg(test)]
mod tests;

pub(crate) use engine::pdf_to_text_with_bundled_ocr;

#[cfg(feature = "pdf-ocr-bundled")]
pub(crate) use engine::{bundled_ocr_engine, ocr_size_guarded};
#[cfg(feature = "pdf-ocr-bundled")]
pub(crate) use merge::{merge_native_and_ocr_regions_text, normalized_text};
#[cfg(feature = "pdf-ocr-bundled")]
pub(crate) use region::{PositionedText, TextRegion};
