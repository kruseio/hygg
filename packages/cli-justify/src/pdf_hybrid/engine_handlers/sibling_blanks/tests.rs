use super::{
  drop_trailing_blanks_after_sibling_list, line_starts_sibling_list_item,
  out_ends_in_caption_context,
};

#[test]
fn sibling_markers_match_bullets_exactly_and_numbers_by_shape() {
  assert!(line_starts_sibling_list_item("  - first", "  ", "- "));
  assert!(!line_starts_sibling_list_item("- first", "  ", "- "));
  assert!(!line_starts_sibling_list_item("  * first", "  ", "- "));
  assert!(!line_starts_sibling_list_item("  first", "  ", ""));
  assert!(!line_starts_sibling_list_item("  first", "  ", "   "));
  assert!(line_starts_sibling_list_item("  12. next", "  ", "1. "));
  assert!(line_starts_sibling_list_item("  12) next", "  ", "1) "));
  assert!(!line_starts_sibling_list_item("  12) next", "  ", "1. "));
  assert!(!line_starts_sibling_list_item("  12. next", "  ", "- "));
  assert!(!line_starts_sibling_list_item("  12. next", "  ", "A "));
  assert!(!line_starts_sibling_list_item("  item", "  ", "1. "));
  assert!(!line_starts_sibling_list_item("  12", "  ", "1. "));
  assert!(!line_starts_sibling_list_item("  12.next", "  ", "1. "));
}

#[test]
fn page_break_blanks_drop_only_between_sibling_items() {
  let mut out = Vec::new();
  drop_trailing_blanks_after_sibling_list(&mut out, "", "- ");
  assert!(out.is_empty());

  let mut out = vec![String::new()];
  drop_trailing_blanks_after_sibling_list(&mut out, "", "- ");
  assert_eq!(out, [""]);

  let mut out = vec!["1. first".into(), "".into()];
  drop_trailing_blanks_after_sibling_list(&mut out, "", "2. ");
  assert_eq!(out, ["1. first"]);

  let mut out = vec!["1. first".into(), "   continued".into(), "".into()];
  drop_trailing_blanks_after_sibling_list(&mut out, "", "2. ");
  assert_eq!(out, ["1. first", "   continued"]);

  let mut out = vec!["ordinary".into(), "".into()];
  drop_trailing_blanks_after_sibling_list(&mut out, "", "2. ");
  assert_eq!(out, ["ordinary", ""]);

  let mut out = vec!["- first".into(), "different block".into(), "".into()];
  drop_trailing_blanks_after_sibling_list(&mut out, "", "- ");
  assert_eq!(out, ["- first", "different block", ""]);

  let mut separated =
    vec!["- first".into(), "".into(), "  continuation".into(), "".into()];
  drop_trailing_blanks_after_sibling_list(&mut separated, "", "- ");
  assert_eq!(separated.len(), 4);
}

#[test]
fn caption_context_joins_wrapped_front_matter_entries() {
  let mut out = Vec::new();
  assert!(!out_ends_in_caption_context(&mut out));
  out.push(String::new());
  assert!(!out_ends_in_caption_context(&mut out));

  let mut out = vec!["Table 2. 中文 title".into()];
  assert!(out_ends_in_caption_context(&mut out));
  assert_eq!(out.len(), 1);

  out.push(String::new());
  assert!(out_ends_in_caption_context(&mut out));
  assert_eq!(out.len(), 1);

  out.push("continued title".into());
  out.push(String::new());
  assert!(out_ends_in_caption_context(&mut out));
  assert_eq!(out.len(), 2);

  let mut unrelated = vec!["ordinary paragraph".into(), "".into()];
  assert!(!out_ends_in_caption_context(&mut unrelated));
  assert_eq!(unrelated.len(), 2);

  let mut separated = vec!["Table 2. title".into(), "".into(), "prose".into()];
  assert!(!out_ends_in_caption_context(&mut separated));
}
