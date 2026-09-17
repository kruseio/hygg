use super::{
  TocPrefixKind, classify_toc_entry_prefix, is_counter_token,
  is_numeric_counter, is_numeric_section_label, is_title_case_label_word,
  looks_like_named_toc_heading, looks_like_toc_entry_prefix,
  looks_like_toc_section_marker, merge_counter_into_prefix_if_needed,
};

#[test]
fn title_case_and_counters_distinguish_ascii_shapes() {
  for word in ["Title", "(Plate)", "O'neill", "École"] {
    assert!(is_title_case_label_word(word), "{word:?}");
  }
  for word in ["", "()", "title", "TITLE", "Title2", "中"] {
    assert!(!is_title_case_label_word(word), "{word:?}");
  }

  for counter in ["1", "1.2", "1-2", "A", "IV", "(3)"] {
    assert!(is_counter_token(counter), "{counter:?}");
  }
  for counter in ["", "()", "１２", "a", "IY"] {
    assert!(!is_counter_token(counter), "{counter:?}");
  }
  for counter in ["1", "1.2", "(3)"] {
    assert!(is_numeric_counter(counter), "{counter:?}");
  }
  for counter in ["", "()", "A", "IV", "1x", ".."] {
    assert!(!is_numeric_counter(counter), "{counter:?}");
  }
}

#[test]
fn prefix_classification_rejects_ambiguous_names() {
  assert!(matches!(
    classify_toc_entry_prefix("1.2"),
    Some(TocPrefixKind::NumericSection)
  ));
  assert!(matches!(
    classify_toc_entry_prefix("Chapter 1"),
    Some(TocPrefixKind::Keyworded)
  ));
  assert!(matches!(
    classify_toc_entry_prefix("Plate 14"),
    Some(TocPrefixKind::TitleNumber)
  ));
  for rejected in [
    "",
    "word",
    "...",
    "Akrom K",
    "Marius IV",
    "Table １２",
    "Chapter abc",
    "Plate ..",
    "Plate 14 extra",
  ] {
    assert!(!looks_like_toc_entry_prefix(rejected), "{rejected:?}");
  }

  for section in ["1", "1.2", "1.2:", "2)"] {
    assert!(is_numeric_section_label(section), "{section:?}");
  }
  for section in ["", ":", "...", "a1", "１"] {
    assert!(!is_numeric_section_label(section), "{section:?}");
  }
}

#[test]
fn headings_and_merge_require_a_counter_and_title() {
  assert!(looks_like_named_toc_heading("Chapter 1 Introduction"));
  for rejected in ["", "Chapter", "chapter 1", "Chapter word"] {
    assert!(!looks_like_named_toc_heading(rejected), "{rejected:?}");
  }
  assert_eq!(
    merge_counter_into_prefix_if_needed("Chapter 1   ", "中文 title"),
    Some(("Chapter 1   ".into(), "中文 title".into()))
  );
  assert_eq!(
    merge_counter_into_prefix_if_needed("Chapter   ", "1 中文 title"),
    Some(("Chapter   1 ".into(), "中文 title".into()))
  );
  for (prefix, title) in [
    ("chapter  ", "1 title"),
    ("Chapter  ", ""),
    ("Chapter  ", "word title"),
    ("Chapter  ", "1"),
  ] {
    assert_eq!(merge_counter_into_prefix_if_needed(prefix, title), None);
  }
}

#[test]
fn section_markers_need_nonempty_ascii_parts() {
  for marker in ["1.2", "A.1", "1.2.3"] {
    assert!(looks_like_toc_section_marker(marker), "{marker:?}");
  }
  for marker in ["", "1", ".1", "1.", "中.2", "1.２", "1.-2"] {
    assert!(!looks_like_toc_section_marker(marker), "{marker:?}");
  }
}
