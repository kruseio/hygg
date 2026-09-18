use super::{
  ListMarkerKind, is_list_continuation_line, parse_format_specifier,
  parse_list_marker, parse_list_marker_with_kind, parse_option_flag,
};

#[test]
fn bullet_number_option_and_format_rows_keep_their_marker_kind() {
  for (line, marker, kind) in [
    ("  • 中文", "• ", ListMarkerKind::Bullet),
    ("  - 中文", "- ", ListMarkerKind::Bullet),
    ("  * 中文", "* ", ListMarkerKind::Bullet),
    ("  ◦ 中文", "◦ ", ListMarkerKind::Bullet),
    ("  -p Description", "-p ", ListMarkerKind::OptionFlag),
    ("  --name-only Description", "--name-only ", ListMarkerKind::OptionFlag),
    ("  %H Commit hash", "%H ", ListMarkerKind::FormatSpecifier),
    ("  %an Author name", "%an ", ListMarkerKind::FormatSpecifier),
    ("  12) Item", "12) ", ListMarkerKind::Numbered),
    ("  1. Item", "1. ", ListMarkerKind::Numbered),
  ] {
    let parsed = parse_list_marker_with_kind(line).unwrap();
    assert_eq!(parsed.0, "  ", "{line}");
    assert_eq!(parsed.1, marker, "{line}");
    assert_eq!(parsed.3, kind, "{line}");
    assert_eq!(parse_list_marker(line).unwrap().2, parsed.2);
  }
}

#[test]
fn marker_lookalikes_do_not_turn_prose_into_lists() {
  for line in [
    "ordinary prose",
    "-3 negative number",
    "-- option description",
    "-p",
    "-p   ",
    "%",
    "%3 invalid",
    "%H",
    "%H   ",
    "1",
    "1:",
    "1.X title",
    "１２. fullwidth number",
  ] {
    assert_eq!(parse_list_marker_with_kind(line), None, "{line}");
  }
}

#[test]
fn individual_option_and_format_tokens_require_ascii_letters() {
  assert_eq!(parse_option_flag("-p Description"), Some("-p"));
  assert_eq!(parse_option_flag("--abbrev-1 Description"), Some("--abbrev-1"));
  assert_eq!(parse_option_flag("foo"), None);
  assert_eq!(parse_option_flag("-"), None);
  assert_eq!(parse_option_flag("-3"), None);
  assert_eq!(parse_format_specifier("%an Author"), Some("%an"));
  assert_eq!(parse_format_specifier("foo"), None);
  assert_eq!(parse_format_specifier("%"), None);
  assert_eq!(parse_format_specifier("%3"), None);
}

#[test]
fn continuation_lines_need_the_list_indent_or_lowercase_text() {
  assert!(!is_list_continuation_line("", "  ", "- "));
  assert!(!is_list_continuation_line("  - new item", "  ", "- "));
  assert!(!is_list_continuation_line("Table 2. caption", "  ", "- "));
  assert!(is_list_continuation_line(
    "    Continuation of the previous item and its explanation",
    "  ",
    "- "
  ));
  assert!(is_list_continuation_line("  continued", "  ", "- "));
  assert!(!is_list_continuation_line("  Continued", "  ", "- "));
  assert!(!is_list_continuation_line("Continued", "  ", "- "));
}
