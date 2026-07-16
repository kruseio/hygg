#![cfg(any(feature = "pdf-rendering", feature = "ocr", test))]

use crate::stream::types::{PdfRegion, VisualTextRow};

pub(crate) fn has_nearby_figure_caption(
  region: PdfRegion,
  native_rows: &[VisualTextRow],
) -> bool {
  native_rows.iter().any(|row| {
    is_figure_caption(&row.text)
      && row.left <= region.left + region.width + 80.0
      && row.left + row.text.chars().count() as f32 * 5.0 >= region.left - 80.0
      && vertical_distance_to_region(region, row.top) <= 90.0
  })
}

pub(crate) fn has_native_text_inside_region(
  region: PdfRegion,
  native_rows: &[VisualTextRow],
) -> bool {
  native_rows.iter().any(|row| {
    !is_figure_caption(&row.text)
      && visual_alnum_len(&row.text) >= 2
      && visual_text_row_overlaps_region(row, region)
  })
}

fn visual_alnum_len(text: &str) -> usize {
  text.chars().filter(|ch| ch.is_alphanumeric()).count()
}

pub(crate) fn is_figure_caption(text: &str) -> bool {
  let trimmed = text.trim_start();
  let Some(rest) = trimmed.strip_prefix("Figure ") else {
    return false;
  };
  rest.chars().next().is_some_and(|ch| ch.is_ascii_digit())
}

fn vertical_distance_to_region(region: PdfRegion, y: f32) -> f32 {
  if y < region.bottom {
    region.bottom - y
  } else if y > region.top() {
    y - region.top()
  } else {
    0.0
  }
}

pub(crate) fn visual_text_row_overlaps_region(
  row: &VisualTextRow,
  region: PdfRegion,
) -> bool {
  let right = region.left + region.width;
  let row_right = row.left + row.text.chars().count() as f32 * 5.0;
  row.top <= region.top() + 6.0
    && row.top >= region.bottom - 6.0
    && row.left <= right + 6.0
    && row_right >= region.left - 6.0
}
