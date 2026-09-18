use cli_justify::{justify, justify_pdf_hybrid, wrap_preserve_whitespace};
use proptest::prelude::*;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

fn grapheme() -> impl Strategy<Value = &'static str> {
  // Escaped zero-width and bidi controls must survive reflow unchanged.
  prop::sample::select(vec![
    "a",
    "Z",
    "é",
    "e\u{301}",
    "中",
    "文",
    "ア",
    "한",
    "Ａ",
    "９",
    "👩‍💻",
    "🇮🇸",
    "\u{200b}",
    "\u{202e}",
    "\u{2066}",
    "\u{2069}",
  ])
}

fn word() -> impl Strategy<Value = String> {
  prop::collection::vec(grapheme(), 1..8).prop_map(|parts| parts.concat())
}

fn non_whitespace(text: &str) -> String {
  text.chars().filter(|ch| !ch.is_whitespace()).collect()
}

fn widest_grapheme(text: &str) -> usize {
  text.graphemes(true).map(UnicodeWidthStr::width).max().unwrap_or(0)
}

proptest! {
  #![proptest_config(ProptestConfig::with_cases(64))]

  #[test]
  fn justification_preserves_the_exact_non_whitespace_stream(
    words in prop::collection::vec(word(), 0..20),
    separator in prop::sample::select(vec![" ", "  ", "\t", "\n"]),
    width in 0usize..33,
  ) {
    let input = words.join(separator);
    let output = justify(&input, width);
    prop_assert_eq!(non_whitespace(&output.concat()), non_whitespace(&input));
    let limit = width.max(1).max(widest_grapheme(&input));
    prop_assert!(output.iter().all(|line| line.width() <= limit), "{input:?} -> {output:?}, width {width}");
  }

  #[test]
  fn whitespace_wrapping_preserves_unicode_and_respects_columns(
    words in prop::collection::vec(word(), 1..16),
    indent in 0usize..18,
    width in 1usize..33,
  ) {
    let input = format!("{}{}", " ".repeat(indent), words.join(" "));
    let output = wrap_preserve_whitespace(&input, width);
    prop_assert_eq!(non_whitespace(&output.concat()), non_whitespace(&input));
    let limit = width.max(widest_grapheme(&input));
    prop_assert!(output.iter().all(|line| line.width() <= limit), "{input:?} -> {output:?}, width {width}");
  }

  #[test]
  fn pdf_hybrid_keeps_single_unicode_tokens_intact(
    input in word(),
    width in 1usize..25,
  ) {
    let output = justify_pdf_hybrid(&input, width);
    prop_assert_eq!(non_whitespace(&output.concat()), non_whitespace(&input));
    let limit = width.max(widest_grapheme(&input));
    prop_assert!(output.iter().all(|line| line.width() <= limit), "{input:?} -> {output:?}, width {width}");
  }
}
