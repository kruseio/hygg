pub(crate) fn pdf_x_to_cells(
  x: f32,
  page_left: f32,
  page_width: f32,
  col: usize,
) -> usize {
  if page_width <= 0.0 || col == 0 {
    return 0;
  }
  (((x - page_left).max(0.0) / page_width) * col as f32).round() as usize
}

pub(crate) fn pdf_width_to_cells(
  width: f32,
  page_width: f32,
  col: usize,
) -> usize {
  if page_width <= 0.0 || col == 0 {
    return 0;
  }
  ((width.max(0.0) / page_width) * col as f32).round() as usize
}

// A figure is at most about a page tall. `width_cells` is already derived from
// `bbox_width` against the page width, so an honest image lands near
// `(bbox_height / page_width) * col` — about 1.3 * col rows for a letter page,
// ~104 at the default col=80. 400 leaves roughly fourfold headroom over
// anything a document would ask for.
//
// The cap is here because the ratio below is document data, not a measurement.
// `cm` scaling is arbitrary, so a page may place an image with a hairline width
// and an enormous height: the ratio then runs to 1e12, `as usize` saturates,
// `as u32` at the call site truncates to u32::MAX, and render_half_block asks
// imageops::resize for a 1 x 4.29e9 RGBA buffer — 17 GB, and the process is
// gone. The width was clamped to the terminal; nothing clamped the height.
const MAX_IMAGE_HEIGHT_ROWS: usize = 400;

pub(crate) fn pdf_image_height_rows(
  bbox_width: f32,
  bbox_height: f32,
  width_cells: usize,
) -> usize {
  // Require finite, positive dimensions: `> 0.0` already rejects zero,
  // negative, and NaN, and `is_finite` rejects the infinity that would
  // otherwise pass `> 0.0` and saturate the cast below into a runaway height.
  let usable = bbox_width.is_finite()
    && bbox_height.is_finite()
    && bbox_width > 0.0
    && bbox_height > 0.0
    && width_cells != 0;
  if !usable {
    return 1;
  }
  (((bbox_height / bbox_width) * width_cells as f32).round().max(1.0) as usize)
    .min(MAX_IMAGE_HEIGHT_ROWS)
}
