use crate::justify;
use crate::text_utils::char_len;

use super::alignment::TocAlignmentState;
use super::engine::{PendingAlignedTocRow, PendingPdfBlock};
use super::structure::AlignedTocRow;
use super::wrapping_plain::{
  apply_prefixes, split_last_word, wrap_plain_with_prefix,
};

const MAX_PARAGRAPH_INDENT_CHARS: usize = 12;
const MIN_WORDS_FOR_INDENT_CAP: usize = 6;

pub(super) fn pending_block_ends_with_hyphen(
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

pub(super) fn pending_paragraph_ends_mid_sentence(
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

fn append_pdf_paragraph_fragment(paragraph: &mut String, line: &str) {
  let fragment = line.trim();
  if fragment.is_empty() {
    return;
  }

  if paragraph.is_empty() {
    paragraph.push_str(fragment);
    return;
  }

  if paragraph.ends_with('-')
    && fragment.chars().next().is_some_and(|ch| ch.is_alphabetic())
  {
    let is_compound = word_before_hyphen_is_compound_prefix(paragraph);
    if is_compound {
      // Hard hyphen between two complete words — keep the hyphen and join
      // the fragment without an extra space.
      paragraph.push_str(fragment);
    } else {
      // Soft hyphen from PDF line wrapping — drop the hyphen and join.
      paragraph.pop();
      paragraph.push_str(fragment);
    }
    return;
  }

  paragraph.push(' ');
  paragraph.push_str(fragment);
}

fn word_before_hyphen_is_compound_prefix(paragraph: &str) -> bool {
  let without_hyphen = paragraph.trim_end_matches('-');
  let last_word = without_hyphen
    .rsplit(|ch: char| ch.is_whitespace())
    .next()
    .unwrap_or_default();
  if last_word.is_empty() {
    return false;
  }

  let lower = last_word.to_ascii_lowercase();
  matches!(
    lower.as_str(),
    "cross"
      | "multi"
      | "semi"
      | "anti"
      | "non"
      | "self"
      | "post"
      | "pre"
      | "sub"
      | "super"
      | "ultra"
      | "inter"
      | "intra"
      | "trans"
      | "mid"
      | "mini"
      | "micro"
      | "macro"
      | "mega"
      | "hyper"
      | "hypo"
      | "over"
      | "under"
      | "well"
      | "ill"
      | "low"
      | "high"
      | "full"
      | "half"
      | "all"
      | "co"
      | "ex"
      | "re"
      | "de"
      | "un"
      | "in"
      | "on"
      | "off"
      | "long"
      | "short"
      | "deep"
      | "left"
      | "right"
      | "two"
      | "one"
      | "open"
      | "free"
      | "real"
      | "user"
      | "page"
      | "line"
      | "file"
      | "code"
      | "data"
      | "web"
      | "side"
      | "front"
      | "back"
      | "top"
      | "end"
      | "form"
      | "name"
      | "node"
      | "type"
      | "case"
      | "word"
      | "size"
      | "time"
      | "year"
      | "site"
      | "view"
      | "test"
      | "team"
      | "tag"
      | "stream"
      | "object"
      | "color"
      | "device"
      | "system"
      | "platform"
      | "network"
      | "language"
      | "encoding"
      | "letter"
      | "english"
      | "latin"
      | "white"
      | "black"
      | "mac"
      | "unix"
      | "x"
      | "y"
      | "z"
      | "m"
      | "n"
      | "t"
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

pub(super) fn wrap_aligned_toc_row(
  row: &AlignedTocRow,
  line_width: usize,
) -> Vec<String> {
  let first_prefix = format!("{}{}", row.indent, row.entry_prefix);
  let continuation_prefix = " ".repeat(char_len(&first_prefix));
  let mut wrapped = wrap_plain_with_prefix(
    &row.title,
    line_width,
    &first_prefix,
    &continuation_prefix,
  );

  let page_suffix = format!("   {}", row.page_number);
  let first_limit = line_width.saturating_sub(char_len(&first_prefix));
  let continuation_limit =
    line_width.saturating_sub(char_len(&continuation_prefix));

  while let Some(last_line) = wrapped.last() {
    let last_idx = wrapped.len() - 1;
    let prefix_len = if last_idx == 0 {
      char_len(&first_prefix)
    } else {
      char_len(&continuation_prefix)
    };
    let usable_width =
      if last_idx == 0 { first_limit } else { continuation_limit };
    let last_text = &last_line[prefix_len..];
    let required = char_len(last_text) + char_len(&page_suffix);
    if required <= usable_width {
      break;
    }

    if let Some((head, tail)) = split_last_word(last_text) {
      wrapped[last_idx] = if last_idx == 0 {
        format!("{first_prefix}{head}")
      } else {
        format!("{continuation_prefix}{head}")
      };
      wrapped.push(format!("{continuation_prefix}{tail}"));
    } else {
      wrapped.push(continuation_prefix.clone());
      break;
    }
  }

  if let Some(last_line) = wrapped.last_mut() {
    last_line.push_str(&page_suffix);
  }

  wrapped
}

pub(super) fn flush_pending_aligned_toc_row(
  pending: &mut Option<PendingAlignedTocRow>,
  out: &mut Vec<String>,
  line_width: usize,
  alignment_state: &mut TocAlignmentState,
) {
  let Some(row) = pending.take() else {
    return;
  };

  let mut row = AlignedTocRow {
    indent: row.indent,
    entry_prefix: row.entry_prefix,
    title: row.title,
    page_number: String::new(),
  };
  alignment_state.normalize_row(&mut row);

  let first_prefix = format!("{}{}", row.indent, row.entry_prefix);
  let continuation_prefix = " ".repeat(char_len(&first_prefix));
  out.extend(wrap_plain_with_prefix(
    &row.title,
    line_width,
    &first_prefix,
    &continuation_prefix,
  ));
}

pub(super) fn flush_pending_pdf_block(
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
          super::structure::looks_like_table_or_figure_caption(first.trim())
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
