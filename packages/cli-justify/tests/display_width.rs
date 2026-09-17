use cli_justify::{justify, justify_pdf_hybrid, wrap_preserve_whitespace};
use unicode_width::UnicodeWidthStr;

#[test]
fn cjk_uses_two_terminal_columns() {
  for text in ["中文测试中文", "日本語の文章", "한국어문장", "ＡＢＣＤ"]
  {
    for width in [2, 3, 4, 7] {
      for lines in [justify(text, width), wrap_preserve_whitespace(text, width)]
      {
        assert!(lines.iter().all(|line| line.width() <= width), "{lines:?}");
        assert_eq!(lines.concat(), text);
      }
    }
  }
}

#[test]
fn mixed_text_is_justified_by_columns() {
  assert_eq!(
    justify("中文 abc 日本語 def", 10),
    ["中文   abc", "日本語 def", ""]
  );
  assert_eq!(wrap_preserve_whitespace("中文 abc def", 9), ["中文 abc", "def"]);
}

#[test]
fn wrapping_keeps_graphemes_intact() {
  for text in ["e\u{301}e\u{301}", "👩‍💻👩‍💻"] {
    let first = if text.starts_with('e') { "e\u{301}" } else { "👩‍💻" };
    let width = first.width();
    assert_eq!(justify(text, width), [first, first, ""]);
    assert_eq!(wrap_preserve_whitespace(text, width), [first, first]);
  }
}

#[test]
fn narrow_columns_preserve_oversized_graphemes_and_terminate() {
  for width in [0, 1] {
    assert_eq!(justify("中文", width), ["中", "文", ""]);
  }
  assert_eq!(wrap_preserve_whitespace("中文", 1), ["中", "文"]);
}

#[test]
fn indentation_counts_toward_display_width() {
  let lines = wrap_preserve_whitespace("    中文日本語한국어", 8);
  assert!(lines.iter().all(|line| line.width() <= 8), "{lines:?}");
  assert_eq!(lines.concat().replace(' ', ""), "中文日本語한국어");
}

#[test]
fn pdf_hybrid_wraps_cjk_by_columns() {
  let lines = justify_pdf_hybrid("中文日本語한국어中文日本語한국어", 8);
  assert!(lines.iter().all(|line| line.width() <= 8), "{lines:?}");
  assert_eq!(
    lines.concat().replace(' ', ""),
    "中文日本語한국어中文日本語한국어"
  );
}
