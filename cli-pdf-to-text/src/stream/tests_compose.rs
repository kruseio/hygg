use super::*;

#[test]
fn visual_composition_orders_text_and_ansi_art_with_metadata() {
  let text_rows = vec![
    VisualTextRow { top: 90.0, left: 50.0, text: "after image".to_string() },
    VisualTextRow { top: 200.0, left: 50.0, text: "before image".to_string() },
  ];
  let image_rows = vec![VisualImageRows {
    top: 150.0,
    left_cells: 4,
    width_cells: 20,
    region: PdfRegion { left: 0.0, bottom: 125.0, width: 100.0, height: 25.0 },
    lines: vec!["\x1b[38;2;1;2;3m\x1b[48;2;4;5;6m▀\x1b[0m".into()],
  }];

  let page = compose_visual_page_events(text_rows, image_rows, 80);

  assert_eq!(
    page.line_kinds,
    vec![PdfLineKind::Text, PdfLineKind::AnsiArt, PdfLineKind::Text,]
  );
  assert_eq!(page.lines[0], "before image");
  assert!(page.lines[1].starts_with("    \x1b[38;2;1;2;3m"));
  assert!(page.lines[1].ends_with("\x1b[0m"));
  assert_eq!(page.lines[2], "after image");
}

#[test]
fn visual_text_inside_image_region_overlays_ansi_art() {
  let text_rows = vec![VisualTextRow {
    top: 140.0,
    left: 25.0,
    text: "diagram label".to_string(),
  }];
  let image_rows = vec![VisualImageRows {
    top: 150.0,
    left_cells: 0,
    width_cells: 40,
    region: PdfRegion { left: 0.0, bottom: 100.0, width: 100.0, height: 50.0 },
    lines: vec![
      format!("\x1b[38;2;1;2;3m{}\x1b[0m", "▀".repeat(40)),
      format!("\x1b[38;2;1;2;3m{}\x1b[0m", "▀".repeat(40)),
    ],
  }];

  let page =
    compose_visual_page_with_overlay(Vec::new(), text_rows, image_rows, 80);

  assert_eq!(page.line_kinds, vec![PdfLineKind::AnsiArt, PdfLineKind::AnsiArt]);
  assert!(
    page.lines.iter().any(|line| line.contains("diagram label")),
    "text should be painted into the ANSI art lines: {:?}",
    page.lines
  );
}

#[test]
fn visual_text_outside_image_region_stays_separate() {
  let text_rows = vec![VisualTextRow {
    top: 75.0,
    left: 25.0,
    text: "caption below".to_string(),
  }];
  let image_rows = vec![VisualImageRows {
    top: 150.0,
    left_cells: 0,
    width_cells: 40,
    region: PdfRegion { left: 0.0, bottom: 100.0, width: 100.0, height: 50.0 },
    lines: vec!["\x1b[38;2;1;2;3m▀▀▀▀▀▀▀▀▀▀\x1b[0m".into()],
  }];

  let page = compose_visual_page(text_rows, image_rows, 80);

  assert_eq!(page.line_kinds, vec![PdfLineKind::AnsiArt, PdfLineKind::Text]);
  assert_eq!(page.lines[1], "caption below");
}

#[test]
fn text_only_ansi_page_keeps_every_line_text_marked() {
  let page = text_only_page_lines("one two three", 10);

  assert!(!page.lines.is_empty());
  assert_eq!(page.line_kinds, vec![PdfLineKind::Text; page.lines.len()]);
}

#[test]
fn visual_page_without_art_uses_native_rows_before_sanitized_fallback() {
  let text_rows = vec![VisualTextRow {
    top: 100.0,
    left: 20.0,
    text: "diagram label".to_string(),
  }];

  let page = compose_visual_page(text_rows, Vec::new(), 80);

  assert_eq!(page.lines, vec!["diagram label"]);
  assert_eq!(page.line_kinds, vec![PdfLineKind::Text]);
}

#[test]
fn visual_row_wrapping_does_not_emit_blank_paragraph_marker() {
  let text_rows = vec![VisualTextRow {
    top: 100.0,
    left: 20.0,
    text: "one two three four five six seven".to_string(),
  }];

  let page = compose_visual_page(text_rows, Vec::new(), 12);

  assert!(
    page.lines.iter().all(|line| !line.trim().is_empty()),
    "wrapping one visual row should not insert blank lines: {:?}",
    page.lines
  );
}

#[test]
fn tiny_pdf_word_gaps_join_same_word_fragments() {
  let mut body = "knowi".to_string();
  push_pdf_word_gap(&mut body, Some(10.0), 10.8, PDF_TEXT_PT_PER_CHAR);
  body.push_str("ng");

  assert_eq!(body, "knowing");

  push_pdf_word_gap(&mut body, Some(20.0), 33.0, PDF_TEXT_PT_PER_CHAR);
  body.push_str("next");

  assert_eq!(body, "knowing   next");
}
