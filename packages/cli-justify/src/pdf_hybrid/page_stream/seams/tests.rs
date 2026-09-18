use super::{
  inter_page_blank_count, line_starts_sibling_list_item, prior_is_caption,
  prior_is_sibling_list_item,
};

fn lines(items: &[&str]) -> Vec<String> {
  items.iter().map(|item| (*item).to_string()).collect()
}

#[test]
fn page_seams_join_lists_captions_and_git_graphs() {
  assert_eq!(inter_page_blank_count(&lines(&["one"]), &[]), 1);
  assert_eq!(inter_page_blank_count(&lines(&["one"]), &lines(&[""])), 1);
  assert_eq!(inter_page_blank_count(&lines(&["one"]), &lines(&["two"])), 1);
  assert_eq!(
    inter_page_blank_count(&lines(&["1. one"]), &lines(&["2. two"])),
    0
  );
  assert_eq!(
    inter_page_blank_count(&lines(&["plain"]), &lines(&["1. new"])),
    1
  );
  assert_eq!(
    inter_page_blank_count(
      &lines(&["Table 1. first"]),
      &lines(&["Table 2. next"])
    ),
    0
  );
  assert_eq!(
    inter_page_blank_count(&lines(&["plain"]), &lines(&["Table 2. next"])),
    1
  );
  assert_eq!(inter_page_blank_count(&lines(&["* |"]), &lines(&["|/"])), 0);
  assert_eq!(inter_page_blank_count(&lines(&["plain"]), &lines(&["|/"])), 1);
}

#[test]
fn prior_list_scan_stops_at_blanks_and_unindented_prose() {
  assert!(!prior_is_sibling_list_item(&[], "", "2. "));
  assert!(prior_is_sibling_list_item(&lines(&["1. one"]), "", "2. "));
  assert!(prior_is_sibling_list_item(
    &lines(&["1. one", "   wrapped continuation"]),
    "",
    "2. "
  ));
  assert!(!prior_is_sibling_list_item(
    &lines(&["1. one", "", "   wrapped continuation"]),
    "",
    "2. "
  ));
  assert!(!prior_is_sibling_list_item(
    &lines(&["1. one", "unrelated prose"]),
    "",
    "2. "
  ));
}

#[test]
fn prior_caption_scan_stops_at_a_blank_boundary() {
  assert!(!prior_is_caption(&[]));
  assert!(prior_is_caption(&lines(&["Table 2. 中文 title"])));
  assert!(prior_is_caption(&lines(&[
    "Table 2. 中文 title",
    "wrapped title continuation",
  ])));
  assert!(!prior_is_caption(&lines(&["Table 2. title", ""])));
  assert!(!prior_is_caption(&lines(&["ordinary prose"])));
}

#[test]
fn sibling_shape_requires_matching_indent_and_punctuation() {
  assert!(line_starts_sibling_list_item("  - one", "  ", "- "));
  assert!(line_starts_sibling_list_item("  12. two", "  ", "1. "));
  assert!(line_starts_sibling_list_item("  12) two", "  ", "1) "));
  assert!(!line_starts_sibling_list_item("- one", "  ", "- "));
  assert!(!line_starts_sibling_list_item("  one", "  ", ""));
  assert!(!line_starts_sibling_list_item("  one", "  ", "   "));
  assert!(!line_starts_sibling_list_item("  12. two", "  ", "- "));
  assert!(!line_starts_sibling_list_item("  12. two", "  ", "1) "));
  assert!(!line_starts_sibling_list_item("  one", "  ", "1. "));
  assert!(!line_starts_sibling_list_item("  12", "  ", "1. "));
  assert!(!line_starts_sibling_list_item("  12.two", "  ", "1. "));
}
