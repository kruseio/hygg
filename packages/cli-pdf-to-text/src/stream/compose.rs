use crate::stream::overlay::overlay_text_rows_on_images;
use crate::stream::types::{
  PdfLineKind, PdfPageForAnsi, VisualImageRows, VisualTextRow,
};

#[cfg(test)]
pub(crate) fn compose_visual_page(
  text_rows: Vec<VisualTextRow>,
  image_rows: Vec<VisualImageRows>,
  col: usize,
) -> PdfPageForAnsi {
  let overlay_text_rows = text_rows.clone();
  compose_visual_page_with_overlay(
    text_rows,
    overlay_text_rows,
    image_rows,
    col,
  )
}

pub(crate) fn compose_visual_page_with_overlay(
  text_rows: Vec<VisualTextRow>,
  overlay_text_rows: Vec<VisualTextRow>,
  mut image_rows: Vec<VisualImageRows>,
  col: usize,
) -> PdfPageForAnsi {
  let _ = overlay_text_rows_on_images(overlay_text_rows, &mut image_rows);
  compose_visual_page_events(text_rows, image_rows, col)
}

pub(crate) fn compose_visual_page_events(
  text_rows: Vec<VisualTextRow>,
  image_rows: Vec<VisualImageRows>,
  col: usize,
) -> PdfPageForAnsi {
  enum Event {
    Text(VisualTextRow),
    Image(VisualImageRows),
  }

  let mut events: Vec<Event> =
    Vec::with_capacity(text_rows.len() + image_rows.len());
  events.extend(text_rows.into_iter().map(Event::Text));
  events.extend(image_rows.into_iter().map(Event::Image));
  events.sort_by(|a, b| {
    let a_top = match a {
      Event::Text(row) => row.top,
      Event::Image(row) => row.top,
    };
    let b_top = match b {
      Event::Text(row) => row.top,
      Event::Image(row) => row.top,
    };
    b_top.partial_cmp(&a_top).unwrap_or(std::cmp::Ordering::Equal)
  });

  let page_left = events
    .iter()
    .filter_map(|event| match event {
      Event::Text(row) if !row.text.trim().is_empty() => Some(row.left),
      _ => None,
    })
    .fold(f32::INFINITY, f32::min);
  let page_left = if page_left.is_finite() { page_left } else { 0.0 };

  let mut lines = Vec::new();
  let mut line_kinds = Vec::new();
  for event in events {
    match event {
      Event::Text(row) => {
        if row.text.trim().is_empty() {
          continue;
        }
        let indent =
          (((row.left - page_left) / 5.0).round()).clamp(0.0, 20.0) as usize;
        let text_width = col.saturating_sub(indent).max(1);
        let mut wrapped_lines = if row.text.chars().count() <= text_width {
          vec![row.text]
        } else {
          cli_justify::justify(&row.text, text_width)
        };
        if wrapped_lines.last().is_some_and(|line| line.is_empty()) {
          wrapped_lines.pop();
        }
        for wrapped in wrapped_lines {
          lines.push(format!("{}{}", " ".repeat(indent), wrapped));
          line_kinds.push(PdfLineKind::Text);
        }
      }
      Event::Image(row) => {
        let indent = " ".repeat(row.left_cells);
        for line in row.lines {
          lines.push(format!("{indent}{line}\x1b[0m"));
          line_kinds.push(PdfLineKind::AnsiArt);
        }
      }
    }
  }

  if lines.is_empty() {
    lines.push(String::new());
    line_kinds.push(PdfLineKind::Text);
  }

  PdfPageForAnsi { lines, line_kinds }
}
