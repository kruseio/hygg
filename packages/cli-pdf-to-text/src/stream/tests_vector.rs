use super::*;

#[test]
fn visual_text_rows_preserve_native_diagram_labels() {
  let rows = vec![
    VisualTextRow { top: 800.0, left: 300.0, text: "12".to_string() },
    VisualTextRow {
      top: 700.0,
      left: 72.0,
      text: "Body text before figure.".to_string(),
    },
    VisualTextRow { top: 660.0, left: 250.0, text: "Acrobat".to_string() },
    VisualTextRow {
      top: 645.0,
      left: 90.0,
      text: "Macintosh application Windows application".to_string(),
    },
    VisualTextRow { top: 630.0, left: 275.0, text: "Adobe PDF".to_string() },
    VisualTextRow { top: 615.0, left: 320.0, text: "printer".to_string() },
    VisualTextRow { top: 600.0, left: 72.0, text: "\u{f05a}".to_string() },
    VisualTextRow {
      top: 560.0,
      left: 72.0,
      text: "Body text after figure.".to_string(),
    },
    VisualTextRow { top: 40.0, left: 300.0, text: "13".to_string() },
  ];

  let filtered = filter_visual_text_rows(rows);
  let texts: Vec<&str> = filtered.iter().map(|row| row.text.as_str()).collect();

  assert_eq!(
    texts,
    vec![
      "Body text before figure.",
      "Acrobat",
      "Macintosh application Windows application",
      "Adobe PDF",
      "printer",
      "Body text after figure.",
    ]
  );
  assert!(filtered.iter().all(|row| row.text.trim() != "\u{f05a}"));
}

#[test]
fn detects_vector_diagram_region_from_box_primitives() {
  let paths = vec![
    pdf_oxide::elements::PathContent::new(pdf_oxide::geometry::Rect::new(
      100.0, 200.0, 80.0, 40.0,
    )),
    pdf_oxide::elements::PathContent::new(pdf_oxide::geometry::Rect::new(
      220.0, 200.0, 80.0, 40.0,
    )),
    pdf_oxide::elements::PathContent::new(pdf_oxide::geometry::Rect::new(
      160.0, 280.0, 80.0, 40.0,
    )),
  ];

  let text_rows = vec![VisualTextRow {
    top: 180.0,
    left: 100.0,
    text: "Figure 1. Test diagram".to_string(),
  }];
  let regions = detect_vector_diagram_regions(
    &paths, 0.0, 0.0, 612.0, 792.0, &text_rows, true,
  );

  assert_eq!(regions.len(), 1);
  assert!(regions[0].left <= 100.0);
  assert!(regions[0].bottom <= 200.0);
  assert!(regions[0].width >= 200.0);
  assert!(regions[0].height >= 120.0);
}

#[test]
fn ignores_single_full_width_vector_rule() {
  let paths = vec![pdf_oxide::elements::PathContent::new(
    pdf_oxide::geometry::Rect::new(0.0, 700.0, 612.0, 1.0),
  )];

  assert!(
    detect_vector_diagram_regions(&paths, 0.0, 0.0, 612.0, 792.0, &[], true)
      .is_empty()
  );
}

#[test]
fn ignores_vector_regions_without_nearby_figure_caption() {
  let paths = vec![
    pdf_oxide::elements::PathContent::new(pdf_oxide::geometry::Rect::new(
      100.0, 200.0, 80.0, 40.0,
    )),
    pdf_oxide::elements::PathContent::new(pdf_oxide::geometry::Rect::new(
      220.0, 200.0, 80.0, 40.0,
    )),
    pdf_oxide::elements::PathContent::new(pdf_oxide::geometry::Rect::new(
      160.0, 280.0, 80.0, 40.0,
    )),
  ];

  assert!(
    detect_vector_diagram_regions(&paths, 0.0, 0.0, 612.0, 792.0, &[], true)
      .is_empty()
  );
}

#[test]
fn ignores_unlabeled_vector_regions_without_ocr() {
  let paths = vec![
    pdf_oxide::elements::PathContent::new(pdf_oxide::geometry::Rect::new(
      100.0, 200.0, 80.0, 40.0,
    )),
    pdf_oxide::elements::PathContent::new(pdf_oxide::geometry::Rect::new(
      220.0, 200.0, 80.0, 40.0,
    )),
    pdf_oxide::elements::PathContent::new(pdf_oxide::geometry::Rect::new(
      160.0, 280.0, 80.0, 40.0,
    )),
  ];
  let text_rows = vec![VisualTextRow {
    top: 180.0,
    left: 100.0,
    text: "Figure 1. Test diagram".to_string(),
  }];

  assert!(
    detect_vector_diagram_regions(
      &paths, 0.0, 0.0, 612.0, 792.0, &text_rows, false,
    )
    .is_empty()
  );
}

#[test]
fn keeps_vector_regions_with_native_overlay_text_without_ocr() {
  let paths = vec![
    pdf_oxide::elements::PathContent::new(pdf_oxide::geometry::Rect::new(
      100.0, 200.0, 80.0, 40.0,
    )),
    pdf_oxide::elements::PathContent::new(pdf_oxide::geometry::Rect::new(
      220.0, 200.0, 80.0, 40.0,
    )),
    pdf_oxide::elements::PathContent::new(pdf_oxide::geometry::Rect::new(
      160.0, 280.0, 80.0, 40.0,
    )),
  ];
  let text_rows = vec![
    VisualTextRow {
      top: 180.0,
      left: 100.0,
      text: "Figure 1. Test diagram".to_string(),
    },
    VisualTextRow { top: 220.0, left: 120.0, text: "Native label".to_string() },
  ];

  let regions = detect_vector_diagram_regions(
    &paths, 0.0, 0.0, 612.0, 792.0, &text_rows, false,
  );

  assert_eq!(regions.len(), 1);
}

#[test]
fn vector_diagram_region_clamps_to_media_box_origin() {
  let paths = vec![
    pdf_oxide::elements::PathContent::new(pdf_oxide::geometry::Rect::new(
      110.0, 210.0, 80.0, 40.0,
    )),
    pdf_oxide::elements::PathContent::new(pdf_oxide::geometry::Rect::new(
      230.0, 210.0, 80.0, 40.0,
    )),
    pdf_oxide::elements::PathContent::new(pdf_oxide::geometry::Rect::new(
      170.0, 290.0, 80.0, 40.0,
    )),
  ];

  let text_rows = vec![VisualTextRow {
    top: 206.0,
    left: 110.0,
    text: "Figure 1. Test diagram".to_string(),
  }];
  let regions = detect_vector_diagram_regions(
    &paths, 100.0, 200.0, 500.0, 500.0, &text_rows, true,
  );

  assert_eq!(regions.len(), 1);
  assert!(regions[0].left >= 100.0);
  assert!(regions[0].bottom >= 200.0);
  assert!(regions[0].left <= 110.0);
  assert!(regions[0].bottom <= 210.0);
}

#[test]
fn vector_diagram_region_handles_negative_media_box_origin() {
  let paths = vec![
    pdf_oxide::elements::PathContent::new(pdf_oxide::geometry::Rect::new(
      -290.0, -190.0, 80.0, 40.0,
    )),
    pdf_oxide::elements::PathContent::new(pdf_oxide::geometry::Rect::new(
      -170.0, -190.0, 80.0, 40.0,
    )),
    pdf_oxide::elements::PathContent::new(pdf_oxide::geometry::Rect::new(
      -230.0, -110.0, 80.0, 40.0,
    )),
  ];

  let text_rows = vec![VisualTextRow {
    top: -194.0,
    left: -290.0,
    text: "Figure 1. Test diagram".to_string(),
  }];
  let regions = detect_vector_diagram_regions(
    &paths, -300.0, -200.0, 500.0, 500.0, &text_rows, true,
  );

  assert_eq!(regions.len(), 1);
  assert!(regions[0].left >= -300.0);
  assert!(regions[0].bottom >= -200.0);
  assert!(regions[0].width >= 200.0);
  assert!(regions[0].height >= 120.0);
}

#[test]
fn pdf_cell_mapping_accounts_for_media_box_origin() {
  assert_eq!(pdf_x_to_cells(100.0, 100.0, 500.0, 80), 0);
  assert_eq!(pdf_x_to_cells(350.0, 100.0, 500.0, 80), 40);
  assert_eq!(pdf_width_to_cells(125.0, 500.0, 80), 20);
}

#[test]
fn pdf_image_height_uses_display_bbox_aspect_ratio() {
  assert_eq!(pdf_image_height_rows(100.0, 50.0, 20), 10);
  assert_eq!(pdf_image_height_rows(100.0, 200.0, 20), 40);
  assert_eq!(pdf_image_height_rows(0.0, 200.0, 20), 1);
}
