use crate::text_utils::{char_len, leading_whitespace};

/// Recognises labelled figure / table / plate captions like
///   * `Table 2. Common options to git log`
///   * `Figure 3.1: Anatomy of a commit`
///   * `Table 12 — Numeric type ranges`
///   * `Plate 14 Radial shading effect …`
///
/// These are typographically separate from the surrounding prose in the
/// PDF (italic / bold / extra leading), but pdf_extract gives us only the
/// text. Without forcing a paragraph break here the caption collapses
/// into the trailing sentence of the previous paragraph — and so does the
/// table that follows it. The Plate variant also covers the
/// list-of-plates section in front-matter, where each entry sits on its
/// own line and would otherwise get glued into one re-justified
/// paragraph.
pub(crate) fn looks_like_table_or_figure_caption(trimmed: &str) -> bool {
  let mut words = trimmed.split_whitespace();
  let Some(label) = words.next() else {
    return false;
  };
  if !matches!(label, "Table" | "Figure" | "Plate" | "Diagram") {
    return false;
  }
  let Some(number) = words.next() else {
    return false;
  };
  let number_clean = number.trim_end_matches(['.', ':', ')']);
  if number_clean.is_empty() {
    return false;
  }
  if !number_clean.chars().all(|ch| ch.is_ascii_digit() || ch == '.') {
    return false;
  }
  // Need at least one more token so we don't fire on a bare "Table 2."
  // reference sitting at the end of an unrelated sentence.
  words.next().is_some()
}

pub(crate) fn should_start_new_pdf_paragraph(
  current_indent: &str,
  previous_line: &str,
  line: &str,
) -> bool {
  // Table / figure captions are paragraph-level labels for the figure
  // that follows. Whatever the indent comparison says, treat them as
  // their own paragraph so the caption (and the table beneath it) don't
  // get glued onto the prior sentence.
  if looks_like_table_or_figure_caption(line.trim()) {
    return true;
  }

  let next_indent = leading_whitespace(line);
  if next_indent == current_indent {
    // Same indent ordinarily means "continuation of the same paragraph",
    // but a sequence of `( ... )` PDF literal-string examples breaks that
    // assumption: each example shares the block's indent yet is meant to
    // be displayed as its own item, not glued into one wrapped paragraph.
    // Detect this shape — the previous line in the pending paragraph
    // closes a parenthesised expression and the new line opens a fresh
    // one with the PDF rendering's leading-space convention — and split
    // there instead.
    let prev = previous_line.trim_end();
    let next_trimmed = line.trim_start();
    if prev.ends_with(')') && next_trimmed.starts_with("( ") {
      return true;
    }
    // Multi-line literal-string examples — `( These \` continued by
    // `two strings \` and closed by `are the same . )` on the next
    // line, or `( ... .` closed by a bare `)` underneath — must keep
    // each source line on its own output line. Break the paragraph
    // when the new line opens or closes a `( ... )` example, or when
    // the previous line ended with a backslash continuation.
    if next_trimmed.starts_with("( ")
      || matches!(next_trimmed, "(" | ")" | "( )")
    {
      return true;
    }
    if prev.ends_with('\\') {
      return true;
    }
    return false;
  }

  let current_indent_width = char_len(current_indent);
  let next_indent_width = char_len(next_indent);
  if next_indent_width > current_indent_width {
    let prev = previous_line.trim_end();
    // A trailing colon ends the previous thought just like a period: the
    // line that follows it ("Examples follow:", "Note the following:") is
    // virtually never a continuation of the same sentence, even when it
    // happens to start with '(' or a lowercase letter. Without this the
    // indent-bump check below would glue a code-like example such as
    // `( This is a string )` onto the introductory text.
    if !prev.is_empty() && !prev.ends_with(['.', '?', '!', ':']) {
      let next_trimmed = line.trim_start_matches([' ', '\t']);
      if next_trimmed.is_empty() {
        return true;
      }

      let first = next_trimmed.chars().next().unwrap_or(' ');
      let looks_like_continuation_fragment = first.is_lowercase()
        || matches!(
          first,
          '('
            | ')'
            | ']'
            | '}'
            | ','
            | '.'
            | ':'
            | ';'
            | '!'
            | '?'
            | '-'
            | '—'
            | '–'
            | '/'
            | '\\'
            | '~'
        )
        || next_trimmed.chars().count() <= 4;

      if looks_like_continuation_fragment {
        return false;
      }

      // A small indent bump (1-2 chars) where the previous line ends mid-
      // sentence is almost always a wrapped continuation, often because
      // an inline code or styled run sits on the next visual line with a
      // slightly different left edge.
      let indent_bump = next_indent_width - current_indent_width;
      if indent_bump <= 2 {
        return false;
      }
    }
  }

  true
}
