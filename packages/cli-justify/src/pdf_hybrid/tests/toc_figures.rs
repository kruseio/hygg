use crate::pdf_hybrid::justify_pdf_hybrid;

#[test]
fn preserves_plate_listing_as_separate_aligned_entries() {
  let input = concat!(
    "  Plate 3     Lab color space (\u{201c}Lab Color Spaces,\u{201d} page 250)\n",
    "  Plate 4    Color gamuts (\u{201c}Lab Color Spaces,\u{201d} page 250)\n",
    "  Plate 5    Rendering intents (\u{201c}Rendering Intents,\u{201d} page 260)\n",
  );
  let out = justify_pdf_hybrid(input, 80);
  let lines_starting_with_plate: Vec<_> =
    out.iter().filter(|line| line.trim_start().starts_with("Plate ")).collect();
  assert!(
    lines_starting_with_plate.len() >= 3,
    "expected each Plate entry on its own line, got: {out:?}"
  );
  for line in &lines_starting_with_plate {
    assert!(
      line.matches("Plate ").count() == 1,
      "expected only one Plate per line, got: {line:?}"
    );
  }
}

#[test]
fn pattern_matches_unknown_label_with_numeric_counter() {
  let input = concat!(
    "  Diagram 1   First diagram (\"Overview\", page 12)\n",
    "  Diagram 2   Second diagram (\"Details\", page 14)\n",
    "  Diagram 3   Third diagram (\"Architecture\", page 18)\n",
  );
  let out = justify_pdf_hybrid(input, 80);
  assert!(
    out.iter().filter(|line| line.trim_start().starts_with("Diagram ")).count()
      >= 3,
    "expected each Diagram entry preserved on its own line, got: {out:?}"
  );
}

#[test]
fn pattern_does_not_misdetect_contributor_initials_as_toc() {
  // First-name + last-initial rows from a multi-column contributors page
  // must remain a preserved layout row (with original wide spacing
  // intact), not be split into a TOC entry that would absorb the next
  // row.
  let input = concat!(
    "   Akrom K                         Jon Freed                       Sergey Kuznetsov\n",
    "   Alan D. Salewski                Jonathan                        Severino Lorilla Jr\n",
  );
  let out = justify_pdf_hybrid(input, 80);
  let akrom_line = out
    .iter()
    .find(|line| line.contains("Akrom K"))
    .expect("expected an Akrom K line");
  // The wide gaps between columns must survive (multiple consecutive
  // spaces), which would have been collapsed if the row had been
  // reflowed as a paragraph or TOC title.
  assert!(
    akrom_line.contains("Akrom K   "),
    "expected Akrom K row's column gap to be preserved, got: {akrom_line:?}"
  );
  assert!(
    akrom_line.contains("Jon Freed"),
    "expected Jon Freed on same row as Akrom K, got: {akrom_line:?}"
  );
}

#[test]
fn preserves_figure_and_table_aligned_entries() {
  let input = concat!(
    "  Figure 1   First diagram   12\n",
    "  Table 2    First table   42\n",
    "  Figure 3   Second diagram (see page 88)\n",
  );
  let out = justify_pdf_hybrid(input, 80);
  assert!(
    out
      .iter()
      .any(|line| line.contains("Figure 1") && line.contains("First diagram")),
    "expected Figure 1 line preserved, got: {out:?}"
  );
  assert!(
    out
      .iter()
      .any(|line| line.contains("Table 2") && line.contains("First table")),
    "expected Table 2 line preserved, got: {out:?}"
  );
  assert!(
    out
      .iter()
      .any(|line| line.contains("Figure 3") && line.contains("Second diagram")),
    "expected Figure 3 line preserved, got: {out:?}"
  );
}
