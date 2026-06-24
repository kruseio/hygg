use crate::justify;
use crate::text_utils::char_len;

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

  let first_width = line_width.saturating_sub(char_len(first_prefix));
  let continuation_width =
    line_width.saturating_sub(char_len(continuation_prefix));
  let usable_width = first_width.min(continuation_width);
  if usable_width == 0 {
    return vec![format!("{first_prefix}{paragraph}")];
  }

  let left_align_deeply_indented_block =
    char_len(first_prefix).max(char_len(continuation_prefix)) >= 12;
  if left_align_deeply_indented_block {
    return wrap_plain_with_prefix(
      paragraph,
      line_width,
      first_prefix,
      continuation_prefix,
    );
  }

  let mut wrapped = justify(paragraph, usable_width);
  if wrapped.last().is_some_and(|line| line.is_empty()) {
    wrapped.pop();
  }

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

  if char_len(indent) > MAX_PARAGRAPH_INDENT_CHARS {
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
        format!("{indent}{}", " ".repeat(char_len(&marker)));
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
