use crate::stream::types::{VisualImageRows, VisualTextRow};

pub(crate) fn overlay_text_rows_on_images(
  text_rows: Vec<VisualTextRow>,
  image_rows: &mut [VisualImageRows],
) -> Vec<VisualTextRow> {
  let mut remaining = Vec::new();
  for row in text_rows {
    if !overlay_text_row_on_first_matching_image(&row, image_rows) {
      remaining.push(row);
    }
  }
  remaining
}

fn overlay_text_row_on_first_matching_image(
  row: &VisualTextRow,
  image_rows: &mut [VisualImageRows],
) -> bool {
  for image in image_rows {
    if !image_contains_text_row(image, row) {
      continue;
    }
    let line_idx = image_text_line_index(image, row.top);
    let col_idx = image_text_col_index(image, row.left);
    let Some(line) = image.lines.get_mut(line_idx) else {
      return false;
    };
    *line = overlay_text_on_ansi_line(line, col_idx, row.text.trim());
    return true;
  }
  false
}

fn image_contains_text_row(
  image: &VisualImageRows,
  row: &VisualTextRow,
) -> bool {
  let right = image.region.left + image.region.width;
  let bottom = image.region.bottom;
  let top = image.region.top();
  let vertical_pad = (image.region.height / image.lines.len().max(1) as f32
    * 0.5)
    .clamp(2.0, 6.0);
  row.top <= top + vertical_pad
    && row.top >= bottom - vertical_pad
    && row.left <= right
    && row.left + row.text.chars().count() as f32 * 5.0 >= image.region.left
}

fn image_text_line_index(image: &VisualImageRows, text_top: f32) -> usize {
  if image.lines.is_empty() || image.region.height <= 0.0 {
    return 0;
  }
  let rel = ((image.region.top() - text_top) / image.region.height)
    .clamp(0.0, 0.999_999);
  (rel * image.lines.len() as f32).floor() as usize
}

fn image_text_col_index(image: &VisualImageRows, text_left: f32) -> usize {
  if image.region.width <= 0.0 || image.width_cells == 0 {
    return 0;
  }
  let rel =
    ((text_left - image.region.left) / image.region.width).clamp(0.0, 1.0);
  (rel * image.width_cells as f32).round() as usize
}

fn overlay_text_on_ansi_line(
  line: &str,
  start_col: usize,
  text: &str,
) -> String {
  let available = ansi_visible_width(line).saturating_sub(start_col);
  if available == 0 {
    return line.to_string();
  }
  let text: String =
    text.chars().filter(|ch| !ch.is_control()).take(available).collect();
  if text.is_empty() {
    return line.to_string();
  }
  let overlay_width = text.chars().count();
  let mut out = String::with_capacity(line.len() + text.len() + 8);
  let mut chars = line.chars().peekable();
  let mut visible_col = 0usize;
  let mut inserted = false;

  while let Some(ch) = chars.next() {
    if ch == '\x1b' {
      out.push(ch);
      for next in chars.by_ref() {
        out.push(next);
        if next == 'm' {
          break;
        }
      }
      continue;
    }

    if !inserted && visible_col >= start_col {
      out.push_str("\x1b[0m");
      out.push_str(&text);
      out.push_str("\x1b[0m");
      inserted = true;
    }

    if inserted
      && visible_col >= start_col
      && visible_col < start_col + overlay_width
    {
      visible_col += 1;
      continue;
    }

    out.push(ch);
    visible_col += 1;
  }

  if !inserted {
    out.push_str(&" ".repeat(start_col.saturating_sub(visible_col)));
    out.push_str("\x1b[0m");
    out.push_str(&text);
  }

  out
}

fn ansi_visible_width(line: &str) -> usize {
  let mut chars = line.chars().peekable();
  let mut width = 0usize;
  while let Some(ch) = chars.next() {
    if ch == '\x1b' {
      for next in chars.by_ref() {
        if next == 'm' {
          break;
        }
      }
      continue;
    }
    width += 1;
  }
  width
}
