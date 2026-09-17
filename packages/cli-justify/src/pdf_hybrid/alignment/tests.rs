use super::{
  TocAlignmentState, is_appendix_subsection_marker,
  is_chapter_like_toc_heading, is_numeric_toc_section_marker,
  plate_entry_marker, toc_section_marker, update_numeric_toc_layout,
};
use crate::pdf_hybrid::structure::AlignedTocRow;

fn row(indent: usize, prefix: &str) -> AlignedTocRow {
  AlignedTocRow {
    indent: " ".repeat(indent),
    entry_prefix: prefix.to_string(),
    title: "中文 title".to_string(),
    page_number: "12".to_string(),
  }
}

#[test]
fn toc_markers_require_ascii_number_shapes() {
  for (marker, expected) in [
    ("1.2", true),
    ("1", false),
    (".2", false),
    ("1.", false),
    ("Ａ.2", false),
    ("1.二", false),
  ] {
    assert_eq!(is_numeric_toc_section_marker(marker), expected, "{marker}");
  }
  for (marker, expected) in [
    ("A.2", true),
    ("A", false),
    ("AB.2", false),
    ("a.2", false),
    ("A.", false),
    ("A.２", false),
  ] {
    assert_eq!(is_appendix_subsection_marker(marker), expected, "{marker}");
  }

  assert_eq!(toc_section_marker(""), None);
  assert_eq!(toc_section_marker("中文 title"), None);
  assert_eq!(toc_section_marker("1.2   中文"), Some("1.2"));
  assert_eq!(plate_entry_marker(""), None);
  assert_eq!(plate_entry_marker("Plate"), None);
  assert_eq!(plate_entry_marker("Figure 1"), None);
  assert_eq!(plate_entry_marker("Plate ２"), None);
  assert_eq!(plate_entry_marker("Plate 12   "), Some("Plate 12".into()));
}

#[test]
fn chapter_and_blank_prefixes_follow_the_canonical_title_column() {
  let mut state = TocAlignmentState::new();
  let mut chapter = row(21, "Chapter 1   ");
  assert!(is_chapter_like_toc_heading(&chapter));
  state.normalize_row(&mut chapter);
  assert_eq!(chapter.indent.len(), 20);

  let mut short = row(2, "Appendix A   ");
  state.normalize_row(&mut short);
  assert_eq!(short.indent.len(), 2);

  let mut blank = row(21, "   ");
  state.normalize_row(&mut blank);
  assert_eq!(blank.indent.len(), 21);

  state.canonical_numeric_toc_layout = Some((5, 14));
  let mut aligned = row(12, "Chapter 2   ");
  state.normalize_row(&mut aligned);
  assert_eq!(aligned.indent.len(), 13);
  let mut distant = row(2, "Chapter 3   ");
  state.normalize_row(&mut distant);
  assert_eq!(distant.indent.len(), 2);
  let mut blank = row(12, "   ");
  state.normalize_row(&mut blank);
  assert_eq!(blank.indent.len(), 13);
}

#[test]
fn plate_labels_align_without_changing_wide_indents() {
  let mut state = TocAlignmentState::new();
  let mut compact = row(0, "Plate 1  ");
  state.normalize_row(&mut compact);
  assert_eq!(compact.entry_prefix, "Plate 1      ");

  let mut deep = row(4, "Plate 12  ");
  state.normalize_row(&mut deep);
  assert_eq!(deep.entry_prefix, "Plate 12        ");

  let mut too_deep = row(20, "Plate 1  ");
  state.normalize_row(&mut too_deep);
  assert_eq!(too_deep.entry_prefix, "Plate 1  ");
}

#[test]
fn numeric_layout_prefers_the_leftmost_tight_title_column() {
  let mut best = None;
  update_numeric_toc_layout(&mut best, 6, 22);
  assert_eq!(best, Some((6, 22)));
  update_numeric_toc_layout(&mut best, 7, 23);
  assert_eq!(best, Some((6, 22)));
  update_numeric_toc_layout(&mut best, 5, 22);
  assert_eq!(best, Some((5, 22)));
  update_numeric_toc_layout(&mut best, 8, 14);
  assert_eq!(best, Some((8, 14)));
  update_numeric_toc_layout(&mut best, 9, 14);
  assert_eq!(best, Some((8, 14)));
}

#[test]
fn numeric_and_appendix_rows_establish_and_follow_alignment() {
  let mut state = TocAlignmentState::new();
  let mut numeric = row(3, "1.2         ");
  state.normalize_row(&mut numeric);
  assert_eq!(state.canonical_numeric_toc_layout, Some((5, 14)));
  assert_eq!(numeric.indent.len(), 3);

  let mut appendix = row(2, "A.3          ");
  state.normalize_row(&mut appendix);
  assert_eq!(appendix.indent.len(), 3);
  assert_eq!(appendix.entry_prefix, "A.3       ");

  let mut nonnumeric = row(2, "AB.1");
  state.normalize_row(&mut nonnumeric);
  assert_eq!(nonnumeric.entry_prefix, "AB.1 ");
}

#[test]
fn a_numeric_row_one_column_too_far_right_is_shifted_left() {
  let mut state = TocAlignmentState::new();
  let mut shifted = row(1, &format!("1.1{}", " ".repeat(17)));
  state.normalize_row(&mut shifted);
  assert_eq!(shifted.indent.len(), 0);
  assert_eq!(state.canonical_numeric_toc_layout, Some((2, 21)));

  let mut state = TocAlignmentState::new();
  let mut at_margin = row(0, &format!("1.1{}", " ".repeat(18)));
  state.normalize_row(&mut at_margin);
  assert_eq!(at_margin.indent.len(), 0);
  assert_eq!(state.canonical_numeric_toc_layout, Some((2, 22)));
}

#[test]
fn numeric_alignment_handles_tight_gaps_and_long_markers() {
  let mut state = TocAlignmentState::new();
  let mut tight = row(2, "1.2 ");
  state.normalize_row(&mut tight);
  assert_eq!(tight.indent.len(), 3);
  assert_eq!(tight.entry_prefix, "1.2 ");

  let mut state = TocAlignmentState::new();
  let mut long = row(1, "123456789012.34 ");
  state.normalize_row(&mut long);
  assert!(long.entry_prefix.starts_with("123456789012.34"));

  let mut state = TocAlignmentState::new();
  let mut wider_number = row(4, "12.3  ");
  state.normalize_row(&mut wider_number);
  assert_eq!(wider_number.indent.len(), 4);
}

#[test]
fn appendix_alignment_handles_missing_or_distant_numeric_reference() {
  let mut state = TocAlignmentState::new();
  let mut first = row(3, "A.2  ");
  state.normalize_row(&mut first);
  assert_eq!(state.canonical_numeric_toc_layout, None);

  state.canonical_numeric_toc_layout = Some((5, 14));
  let mut distant = row(10, "A.3  ");
  state.normalize_row(&mut distant);
  assert_eq!(distant.indent.len(), 10);

  let mut wide_gap = row(3, &format!("A.4{}", " ".repeat(20)));
  state.normalize_row(&mut wide_gap);
  assert!(wide_gap.entry_prefix.len() > 20);
}

#[test]
fn generic_section_rows_track_prefix_width_without_large_jumps() {
  let mut state = TocAlignmentState::new();
  let mut first = row(2, "AB.1  ");
  state.normalize_row(&mut first);
  let mut wider = row(3, "AB.1  ");
  state.normalize_row(&mut wider);
  let mut drifted = row(2, "AB.1  ");
  state.normalize_row(&mut drifted);
  assert_eq!(drifted.indent.len(), 3);

  let mut far = row(0, "AB.1  ");
  state.normalize_row(&mut far);
  assert_eq!(far.indent.len(), 0);

  let mut unknown = row(3, "Unicode title");
  state.normalize_row(&mut unknown);
  assert_eq!(unknown.indent.len(), 3);
}
