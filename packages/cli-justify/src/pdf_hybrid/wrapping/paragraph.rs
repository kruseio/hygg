use crate::justify;
use crate::text_utils::display_width;

use crate::pdf_hybrid::engine::PendingPdfBlock;
use crate::pdf_hybrid::wrapping::hyphenation::append_pdf_paragraph_fragment;
use crate::pdf_hybrid::wrapping_plain::{
  apply_prefixes, wrap_plain_with_prefix,
};

const MAX_PARAGRAPH_INDENT_CHARS: usize = 12;
const MIN_WORDS_FOR_INDENT_CAP: usize = 6;

pub(crate) fn pending_block_ends_with_hyphen(
  pending: &Option<PendingPdfBlock>,
) -> bool {
  match pending {
    Some(PendingPdfBlock::Paragraph { lines, .. })
    | Some(PendingPdfBlock::ListItem { lines, .. }) => {
      lines.last().is_some_and(|line| line.trim_end().ends_with('-'))
    }
    None => false,
  }
}

pub(crate) fn pending_paragraph_ends_mid_sentence(
  pending: &Option<PendingPdfBlock>,
) -> bool {
  let Some(PendingPdfBlock::Paragraph { lines, .. }) = pending else {
    return false;
  };
  let Some(last) = lines.last() else {
    return false;
  };
  let trimmed = last.trim_end();
  if trimmed.is_empty() {
    return false;
  }
  let last_char = trimmed.chars().last().unwrap_or(' ');
  !matches!(
    last_char,
    '.' | '?' | '!' | ':' | ';' | ')' | ']' | '}' | '”' | '"' | '\u{2014}'
  )
}

fn collapse_pdf_paragraph_lines(lines: Vec<String>) -> String {
  let mut paragraph = String::new();
  for line in lines {
    append_pdf_paragraph_fragment(&mut paragraph, &line);
  }
  paragraph
}

fn wrap_paragraph_with_prefix(
  paragraph: &str,
  line_width: usize,
  first_prefix: &str,
  continuation_prefix: &str,
) -> Vec<String> {
  if paragraph.is_empty() {
    return Vec::new();
  }

  let first_width = line_width.saturating_sub(display_width(first_prefix));
  let continuation_width =
    line_width.saturating_sub(display_width(continuation_prefix));
  let usable_width = first_width.min(continuation_width);
  if usable_width == 0 {
    return vec![format!("{first_prefix}{paragraph}")];
  }

  let left_align_deeply_indented_block =
    display_width(first_prefix).max(display_width(continuation_prefix)) >= 12;
  if left_align_deeply_indented_block {
    return wrap_plain_with_prefix(
      paragraph,
      line_width,
      first_prefix,
      continuation_prefix,
    );
  }

  let mut wrapped = justify(paragraph, usable_width);
  // `justify` always appends an empty paragraph separator.
  wrapped.pop();

  apply_prefixes(wrapped, first_prefix, continuation_prefix)
}

fn capped_paragraph_indent_width(
  paragraph: &str,
  indent: &str,
) -> Option<usize> {
  let word_count = paragraph.split_whitespace().count();
  if word_count < MIN_WORDS_FOR_INDENT_CAP {
    return None;
  }

  if display_width(indent) > MAX_PARAGRAPH_INDENT_CHARS {
    return Some(MAX_PARAGRAPH_INDENT_CHARS);
  }

  None
}

pub(crate) fn flush_pending_pdf_block(
  pending: &mut Option<PendingPdfBlock>,
  out: &mut Vec<String>,
  line_width: usize,
) -> Option<usize> {
  let block = pending.take()?;

  match block {
    PendingPdfBlock::Paragraph { indent, lines } => {
      let is_caption = lines
        .first()
        .map(|first| {
          crate::pdf_hybrid::structure::looks_like_table_or_figure_caption(
            first.trim(),
          )
        })
        .unwrap_or(false);
      let paragraph = collapse_pdf_paragraph_lines(lines);
      let capped_indent = capped_paragraph_indent_width(&paragraph, &indent);
      let indent = capped_indent.map_or(indent, |width| " ".repeat(width));
      // Caption-style paragraphs (Plate / Table / Figure / Diagram
      // entries in a front-matter list) read as labeled items, not
      // prose. Justifying them inserts extra inter-word spacing that
      // makes a tight list look double-spaced and ragged. Plain wrap
      // gives the expected `Plate 1 … long title …` shape with the
      // overflow word on a continuation line.
      if is_caption {
        out.extend(wrap_plain_with_prefix(
          &paragraph, line_width, &indent, &indent,
        ));
      } else {
        out.extend(wrap_paragraph_with_prefix(
          &paragraph, line_width, &indent, &indent,
        ));
      }
      capped_indent
    }
    PendingPdfBlock::ListItem { indent, marker, lines } => {
      let paragraph = collapse_pdf_paragraph_lines(lines);
      let continuation_prefix =
        format!("{indent}{}", " ".repeat(display_width(&marker)));
      let first_prefix = format!("{indent}{marker}");
      out.extend(wrap_paragraph_with_prefix(
        &paragraph,
        line_width,
        &first_prefix,
        &continuation_prefix,
      ));
      None
    }
  }
}

#[cfg(test)]
mod tests {
  use super::{
    capped_paragraph_indent_width, flush_pending_pdf_block,
    pending_block_ends_with_hyphen, pending_paragraph_ends_mid_sentence,
    wrap_paragraph_with_prefix,
  };
  use crate::pdf_hybrid::engine::PendingPdfBlock;
  use crate::text_utils::display_width;

  fn paragraph(lines: &[&str]) -> Option<PendingPdfBlock> {
    Some(PendingPdfBlock::Paragraph {
      indent: String::new(),
      lines: lines.iter().map(|line| (*line).to_string()).collect(),
    })
  }

  #[test]
  fn paragraph_boundaries_respect_hyphens_and_sentence_endings() {
    assert!(!pending_block_ends_with_hyphen(&None));
    assert!(pending_block_ends_with_hyphen(&paragraph(&["split-  "])));
    assert!(!pending_block_ends_with_hyphen(&paragraph(&[])));
    assert!(pending_block_ends_with_hyphen(&Some(PendingPdfBlock::ListItem {
      indent: String::new(),
      marker: "- ".into(),
      lines: vec!["word-".into()],
    })));

    assert!(!pending_paragraph_ends_mid_sentence(&None));
    assert!(!pending_paragraph_ends_mid_sentence(&paragraph(&[])));
    assert!(!pending_paragraph_ends_mid_sentence(&paragraph(&["   "])));
    assert!(!pending_paragraph_ends_mid_sentence(&paragraph(&["Done.” "])));
    assert!(pending_paragraph_ends_mid_sentence(&paragraph(&[
      "still reading "
    ])));
  }

  #[test]
  fn paragraph_prefixes_handle_empty_narrow_and_deep_layouts() {
    assert!(wrap_paragraph_with_prefix("", 10, "1  ", "   ").is_empty());
    assert_eq!(
      wrap_paragraph_with_prefix("text", 2, "1  ", "   "),
      ["1  text"]
    );

    let deep = wrap_paragraph_with_prefix(
      "中文 title and more words",
      18,
      "            ",
      "            ",
    );
    assert!(deep.iter().all(|line| display_width(line) <= 18));
    assert_eq!(
      deep.concat().split_whitespace().collect::<String>(),
      "中文titleandmorewords"
    );

    let ordinary =
      wrap_paragraph_with_prefix("one two three", 11, "1  ", "   ");
    assert_eq!(ordinary, ["1  one  two", "   three"]);
  }

  #[test]
  fn long_paragraph_indents_are_capped_but_short_ones_are_not() {
    assert_eq!(capped_paragraph_indent_width("one two", "             "), None);
    assert_eq!(
      capped_paragraph_indent_width(
        "one two three four five six",
        "             "
      ),
      Some(12)
    );
    assert_eq!(
      capped_paragraph_indent_width("one two three four five six", "  "),
      None
    );
  }

  #[test]
  fn flushing_paragraphs_and_list_items_preserves_their_roles() {
    let mut out = Vec::new();
    let mut pending = None;
    assert_eq!(flush_pending_pdf_block(&mut pending, &mut out, 24), None);
    assert!(out.is_empty());

    pending = paragraph(&["Table 2. 中文 options"]);
    assert_eq!(flush_pending_pdf_block(&mut pending, &mut out, 24), None);
    assert_eq!(out, ["Table 2. 中文 options"]);

    pending = Some(PendingPdfBlock::Paragraph {
      indent: "             ".into(),
      lines: vec!["one two three four five six seven".into()],
    });
    assert_eq!(flush_pending_pdf_block(&mut pending, &mut out, 24), Some(12));
    assert!(out.last().is_some_and(|line| line.starts_with("            ")));

    pending = Some(PendingPdfBlock::ListItem {
      indent: "  ".into(),
      marker: "- ".into(),
      lines: vec!["a list item with more words".into()],
    });
    assert_eq!(flush_pending_pdf_block(&mut pending, &mut out, 20), None);
    assert!(out.iter().any(|line| line.starts_with("  - ")));
  }
}
