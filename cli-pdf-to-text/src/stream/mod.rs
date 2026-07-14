mod compose;
mod core;
mod geometry;
mod images;
#[cfg(feature = "pdf-ocr-bundled")]
mod ocr;
mod overlay;
mod text_lines;
mod text_rows;
mod types;
mod vector;
#[cfg(any(feature = "pdf-rendering", test))]
mod vector_detect;
#[cfg(any(feature = "pdf-rendering", feature = "pdf-ocr-bundled", test))]
mod vector_geom;

// Public API — preserve the original `stream::...` external paths.
pub use types::{PdfLineKind, PdfRenderedPage, PdfStream, SharedPdfStream};

// Crate-internal re-exports so the `#[cfg(test)] mod tests` below (which
// uses `super::*`) and any other in-crate caller keeps resolving the same
// item names it used when everything lived in one file.
#[cfg(test)]
pub(crate) use compose::compose_visual_page;
pub(crate) use compose::{
  compose_visual_page_events, compose_visual_page_with_overlay,
};
pub(crate) use geometry::{
  pdf_image_height_rows, pdf_width_to_cells, pdf_x_to_cells,
};
// Full-resolution image sources for the rich clients' high-fidelity render.
#[cfg(feature = "visual-assets")]
pub(crate) use images::render_pdf_images_sourced;
pub(crate) use text_lines::{
  extract_page_text_lines, is_digits_only, push_pdf_word_gap,
};
pub(crate) use text_rows::{
  extract_visual_text_rows, filter_visual_text_rows, normalize_visual_text_row,
  positioned_sanitized_text_rows, positioned_visual_text_rows,
  text_only_page_lines,
};
pub(crate) use types::{
  PDF_TEXT_PT_PER_CHAR, PdfPageForAnsi, PdfRegion, VisualImageRows,
  VisualTextRow,
};
pub(crate) use vector::render_vector_diagram_regions;
#[cfg(any(feature = "pdf-rendering", test))]
pub(crate) use vector_detect::detect_vector_diagram_regions;
#[cfg(any(feature = "pdf-rendering", feature = "pdf-ocr-bundled", test))]
pub(crate) use vector_geom::{
  has_nearby_figure_caption, is_figure_caption, visual_text_row_overlaps_region,
};

#[cfg(feature = "pdf-ocr-bundled")]
pub(crate) use ocr::{
  has_near_duplicate_visual_text, normalized_visual_text,
  ocr_dynamic_image_text_rows, ocr_visual_text_rows, should_ocr_image_region,
};

#[cfg(test)]
mod tests_compose;
#[cfg(test)]
mod tests_extract;
#[cfg(all(test, feature = "pdf-ocr-bundled"))]
mod tests_ocr;
#[cfg(test)]
mod tests_reflow;
#[cfg(test)]
mod tests_vector;
