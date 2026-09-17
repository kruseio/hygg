use cli_justify::{justify, justify_pdf_hybrid, wrap_preserve_whitespace};
use unicode_segmentation::UnicodeSegmentation;
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
    assert_eq!(wrap_preserve_whitespace("中文", width), ["中", "文"]);
  }
}

#[test]
fn mixed_graphemes_wrap_without_loss_or_splitting() {
  for text in ["a👩‍💻中文b", "e\u{301}中文👩‍💻", "🇮🇸中文Ａ"]
  {
    let grapheme_boundaries: Vec<usize> = text
      .grapheme_indices(true)
      .map(|(byte_idx, _)| byte_idx)
      .chain(std::iter::once(text.len()))
      .collect();
    let widest_grapheme =
      text.graphemes(true).map(UnicodeWidthStr::width).max().unwrap_or(0);

    for width in 1..=7 {
      for lines in [justify(text, width), wrap_preserve_whitespace(text, width)]
      {
        assert_eq!(lines.concat(), text, "{lines:?}");
        assert!(
          lines.iter().all(|line| line.width() <= width.max(widest_grapheme)),
          "width {width}: {lines:?}"
        );
        let mut end = 0;
        for line in lines {
          end += line.len();
          assert!(grapheme_boundaries.contains(&end), "split at {end}");
        }
      }
    }
  }
}

#[test]
fn exact_width_keeps_a_wide_grapheme_on_the_current_line() {
  assert_eq!(wrap_preserve_whitespace("a中文", 3), ["a中", "文"]);
  assert_eq!(justify("a中文", 3), ["a中", "文", ""]);
}

#[test]
fn public_wrappers_handle_blank_lines_and_boundary_whitespace() {
  assert_eq!(wrap_preserve_whitespace("a\n\nb", 4), ["a", "", "b"]);
  assert_eq!(wrap_preserve_whitespace("          ", 4), [""]);
  assert_eq!(wrap_preserve_whitespace("     中文abc", 10), ["   中文abc"]);
  assert_eq!(wrap_preserve_whitespace("abc ", 3), ["abc"]);
  assert_eq!(
    wrap_preserve_whitespace("abc          ", 5).concat().trim(),
    "abc"
  );

  assert_eq!(justify("a b c d", 6), ["a  b c", "d", ""]);
  assert_eq!(justify(" \n\nx", 10), ["", "x", ""]);

  let heredoc = justify_pdf_hybrid("cat <<EOF\n  中文 sample\nEOF", 40);
  assert!(heredoc.iter().any(|line| line.contains("中文 sample")));
}

#[test]
fn indentation_counts_toward_display_width() {
  let lines = wrap_preserve_whitespace("    中文日本語한국어", 8);
  assert!(lines.iter().all(|line| line.width() <= 8), "{lines:?}");
  assert_eq!(lines.concat().replace(' ', ""), "中文日本語한국어");
}

#[test]
fn tabs_do_not_disappear_from_the_width_budget() {
  // A tab after three columns reaches the next tab stop before `def`.
  assert_eq!(wrap_preserve_whitespace("abc\tdef", 6), ["abc", "def"]);
  let lines = wrap_preserve_whitespace("\t中文", 8);
  assert!(lines.iter().all(|line| !line.contains('\t') && line.width() <= 8));
  assert_eq!(lines.concat().trim(), "中文");
}

#[test]
fn excessive_unicode_indent_is_clamped_without_splitting_the_text() {
  let body = "中文 abc ".repeat(7);
  let input = format!("{}{}", " ".repeat(25), body);
  let expected = format!("{}{}", " ".repeat(80 - body.width()), body);
  assert_eq!(wrap_preserve_whitespace(&input, 80), [expected]);
}

#[test]
fn ordinary_indents_still_wrap_at_the_boundary() {
  let body = format!("{}中文 ab", "中文 abc ".repeat(8));
  assert_eq!(body.width(), 79);
  for indent in [3, 20] {
    let input = format!("{}{}", " ".repeat(indent), body);
    let lines = wrap_preserve_whitespace(&input, 80);
    assert!(lines.len() > 1, "indent {indent}: {lines:?}");
    assert!(lines[0].starts_with(&" ".repeat(indent)));
  }
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

#[test]
fn pdf_toc_handles_unicode_prefix_and_cjk_title() {
  // Mixing narrow multibyte letters with a fullwidth letter makes the
  // prefix's display width land inside the final UTF-8 character.
  let mixed_prefix = "Ééééａé 1   The title and more words   12";
  assert_eq!(justify_pdf_hybrid(mixed_prefix, 80), [mixed_prefix]);

  let wrapped = justify_pdf_hybrid(mixed_prefix, 24);
  assert!(wrapped.iter().all(|line| line.width() <= 24), "{wrapped:?}");
  assert_eq!(
    wrapped.join(" ").split_whitespace().collect::<Vec<_>>(),
    mixed_prefix.split_whitespace().collect::<Vec<_>>()
  );

  let cjk_title = "Chapter 1   中文测试日本語한국어   12";
  let wrapped = justify_pdf_hybrid(cjk_title, 24);
  assert!(wrapped.iter().all(|line| line.width() <= 24), "{wrapped:?}");
  let without_spacing = |text: &str| {
    text.chars().filter(|ch| !ch.is_whitespace()).collect::<String>()
  };
  assert_eq!(without_spacing(&wrapped.concat()), without_spacing(cjk_title));

  let row = "Chapter 1   中文 title   12";
  assert_eq!(
    justify_pdf_hybrid(row, 24),
    ["Chapter 1   中文", "            title   12"]
  );
}

#[test]
fn unfinished_toc_row_does_not_absorb_following_prose() {
  let lines =
    justify_pdf_hybrid("Chapter 1   中文 title\nOrdinary prose follows.", 80);
  assert!(lines.iter().any(|line| line.contains("中文 title")), "{lines:?}");
  assert!(
    lines.iter().any(|line| line.contains("Ordinary prose follows.")),
    "{lines:?}"
  );
  assert!(
    lines
      .iter()
      .all(|line| !(line.contains("中文 title")
        && line.contains("Ordinary prose"))),
    "{lines:?}"
  );
}
