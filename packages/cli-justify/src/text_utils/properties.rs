use super::{
  display_width, split_at_width, split_trailing_numeric_token_with_min_gap,
};
use proptest::prelude::*;
use unicode_segmentation::UnicodeSegmentation;

proptest! {
  #![proptest_config(ProptestConfig::with_cases(64))]

  #[test]
  fn width_splits_reconstruct_the_input_at_grapheme_boundaries(
    parts in prop::collection::vec(
      prop::sample::select(vec!["a", "中", "Ａ", "e\u{301}", "👩‍💻", "🇮🇸", "\u{200b}", "\u{202e}"]),
      0..16,
    ),
    columns in 0usize..25,
  ) {
    let input = parts.concat();
    let (left, right) = split_at_width(&input, columns);
    prop_assert!(input.starts_with(left));
    prop_assert!(
      input.grapheme_indices(true).any(|(index, _)| index == left.len()) || left.len() == input.len(),
      "split inside grapheme: {input:?}, {left:?}"
    );
    if let Some(right) = right {
      prop_assert!(!left.is_empty());
      prop_assert!(!right.is_empty());
      prop_assert_eq!(format!("{left}{right}"), input.as_str());
      prop_assert!(display_width(left) <= columns || left.graphemes(true).count() == 1);
    } else {
      prop_assert_eq!(left, input.as_str());
    }
  }

  #[test]
  fn trailing_ascii_pages_split_only_after_a_wide_enough_gap(
    title_parts in prop::collection::vec(prop::sample::select(vec!["中", "文", "A", "é", "Ａ"]), 1..8),
    gap in 0usize..5,
    min_gap in 1usize..5,
    page in 0u16..1000,
  ) {
    let title = title_parts.concat();
    let page = page.to_string();
    let input = format!("{title}{}{page}", " ".repeat(gap));
    let (left, parsed) = split_trailing_numeric_token_with_min_gap(&input, min_gap);
    if gap >= min_gap {
      prop_assert_eq!(left, title.as_str());
      prop_assert_eq!(parsed, Some(page.as_str()));
    } else {
      prop_assert_eq!(left, input.as_str());
      prop_assert_eq!(parsed, None);
    }
  }
}
