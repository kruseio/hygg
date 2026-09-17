use super::{
  normalize_preserved_compact_layout_line, parse_aligned_toc_continuation,
  parse_aligned_toc_row_start, parse_dot_leader_toc_row,
  parse_plain_aligned_toc_row, split_on_first_wide_gap,
};
use proptest::prelude::*;

#[test]
fn wide_gap_requires_text_on_both_sides() {
  assert_eq!(
    split_on_first_wide_gap("Chapter 1   中文 title"),
    Some(("Chapter 1   ", "中文 title"))
  );
  for rejected in ["", "Chapter 1 中文", "   中文", "Chapter 1   "] {
    assert_eq!(split_on_first_wide_gap(rejected), None, "{rejected:?}");
  }
}

#[test]
fn dot_leader_rows_need_a_title_leader_and_ascii_page() {
  let row = parse_aligned_toc_row_start("  中文 overview .... 12").unwrap();
  assert_eq!(row.indent, "  ");
  assert_eq!(row.title_fragment, "中文 overview");
  assert_eq!(row.page_number.as_deref(), Some("12"));
  for rejected in [
    "中文 overview ... 12",
    "中文 overview .... XII",
    ".... 12",
    "中文 overview .... １２",
  ] {
    assert!(parse_aligned_toc_row_start(rejected).is_none(), "{rejected:?}");
  }
  assert!(parse_dot_leader_toc_row("Title .... XII").is_none());
  assert!(parse_dot_leader_toc_row("").is_none());
  assert!(parse_dot_leader_toc_row("Title....suffix ... 12").is_none());
}

#[test]
fn aligned_rows_require_a_valid_prefix_and_clean_page() {
  let row =
    parse_aligned_toc_row_start("  Chapter 1   中文 title   12").unwrap();
  assert_eq!(row.indent, "  ");
  assert_eq!(row.entry_prefix.trim(), "Chapter 1");
  assert_eq!(row.title_fragment, "中文 title");
  assert_eq!(row.page_number.as_deref(), Some("12"));

  let pending = parse_aligned_toc_row_start("Chapter 1   中文 title").unwrap();
  assert_eq!(pending.page_number, None);

  for rejected in [
    "",
    "Chapter 1 中文 title 12",
    "CAPTION 1   中文 title   12",
    "Unknown label   中文 title   12",
    "            Chapter 1   中文 title",
    "Chapter 1  中文 title",
    "Plate 14   中文 title",
    "Chapter 123456789012345678901234   title   12",
    "Akrom  IV   title",
    "Akrom  IV   title   12",
  ] {
    assert!(parse_aligned_toc_row_start(rejected).is_none(), "{rejected:?}");
  }
}

#[test]
fn continuations_and_plain_rows_reject_non_content() {
  assert_eq!(
    parse_aligned_toc_continuation("  continued 中文 title   14"),
    Some(("continued 中文 title".into(), Some("14".into())))
  );
  assert_eq!(
    parse_aligned_toc_continuation("  continued 中文 title"),
    Some(("continued 中文 title".into(), None))
  );
  assert_eq!(parse_aligned_toc_continuation(" "), None);
  assert_eq!(parse_aligned_toc_continuation("Ordinary prose"), None);
  assert_eq!(
    parse_aligned_toc_continuation("continued 中文   14"),
    Some(("continued 中文".into(), Some("14".into())))
  );

  let row = parse_plain_aligned_toc_row("  中文 overview   12").unwrap();
  assert_eq!(row.indent, "  ");
  assert_eq!(row.title, "中文 overview");
  assert_eq!(row.page_number, "12");
  for rejected in [
    "",
    "中文 overview .... 12",
    "中文 overview",
    "    12",
    "1.2 Overview   12",
    "Chapter 1 Overview   12",
    "12 Overview   14",
  ] {
    assert!(parse_plain_aligned_toc_row(rejected).is_none(), "{rejected:?}");
  }
}

#[test]
fn compact_layout_normalizes_only_labelled_rows() {
  assert_eq!(
    normalize_preserved_compact_layout_line("FIGURE\u{2003}1  中文 title"),
    "FIGURE 1     中文 title"
  );
  for unchanged in [
    "    FIGURE 1  title",
    "FIGURE",
    "verylonglabel 1  title",
    "figure 1  title",
    "F1 1  title",
    "FIGURE A  title",
    "FIGURE 1 title",
    "FIGURE 1  ",
    "LONGCAPTION 12345  title",
  ] {
    assert_eq!(normalize_preserved_compact_layout_line(unchanged), unchanged);
  }
}

proptest! {
  #![proptest_config(ProptestConfig::with_cases(64))]

  #[test]
  fn compact_layout_preserves_content_across_multibyte_spaces(
    label in prop::sample::select(vec!["FIGURE", "TABLE", "Plate", "Ａ", "中文"]),
    gap in prop::sample::select(vec![" ", "\t", "\u{00a0}", "\u{2003}", "\u{3000}"]),
    title in prop::sample::select(vec!["中文 title", "ＡＢ chart", "école", "👩‍💻"]),
    indent in 0usize..5,
    number in 1u16..1000,
    title_gap in 2usize..7,
  ) {
    let input = format!("{}{label}{gap}{number}{}{title}", " ".repeat(indent), " ".repeat(title_gap));
    let normalized = normalize_preserved_compact_layout_line(&input);
    let non_whitespace = |text: &str| text.chars().filter(|ch| !ch.is_whitespace()).collect::<String>();
    prop_assert_eq!(non_whitespace(&normalized), non_whitespace(&input));
    prop_assert_eq!(normalize_preserved_compact_layout_line(&normalized), normalized);
  }
}
