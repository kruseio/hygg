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

pub(crate) fn pdf_image_height_rows(
  bbox_width: f32,
  bbox_height: f32,
  width_cells: usize,
) -> usize {
  if bbox_width <= 0.0 || bbox_height <= 0.0 || width_cells == 0 {
    return 1;
  }
  ((bbox_height / bbox_width) * width_cells as f32).round().max(1.0) as usize
}
