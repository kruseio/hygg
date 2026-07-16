use crate::stream::text_lines::{is_digits_only, push_pdf_word_gap};
use crate::stream::types::{
  PDF_TEXT_PT_PER_CHAR, PdfLineKind, PdfPageForAnsi, VisualTextRow,
};

pub(crate) fn text_only_page_lines(
  raw_text: &str,
  col: usize,
) -> PdfPageForAnsi {
  let lines = cli_justify::justify_pdf_page(raw_text, col).lines;
  let line_kinds = vec![PdfLineKind::Text; lines.len()];
  PdfPageForAnsi { lines, line_kinds }
}

pub(crate) fn positioned_sanitized_text_rows(
  doc: &pdf_oxide::PdfDocument,
  page_0based: usize,
  raw_text: &str,
  col: usize,
) -> Vec<VisualTextRow> {
  let sanitized_lines = cli_justify::justify_pdf_page(raw_text, col).lines;
  let anchors = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    extract_visual_text_rows(doc, page_0based)
  }))
  .ok()
  .flatten()
  .unwrap_or_default();

  if anchors.is_empty() {
    return sanitized_lines
      .into_iter()
      .enumerate()
      .map(|(idx, text)| VisualTextRow { top: -(idx as f32), left: 0.0, text })
      .collect();
  }

  sanitized_lines
    .into_iter()
    .enumerate()
    .map(|(idx, text)| {
      let anchor = anchors
        .get(idx)
        .or_else(|| anchors.last())
        .expect("anchors is non-empty");
      let extra = idx.saturating_sub(anchors.len().saturating_sub(1)) as f32;
      VisualTextRow { top: anchor.top - extra, left: anchor.left, text }
    })
    .collect()
}

pub(crate) fn positioned_visual_text_rows(
  doc: &pdf_oxide::PdfDocument,
  page_0based: usize,
) -> Vec<VisualTextRow> {
  std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    extract_visual_text_rows(doc, page_0based)
  }))
  .ok()
  .flatten()
  .map(filter_visual_text_rows)
  .unwrap_or_default()
}

pub(crate) fn filter_visual_text_rows(
  rows: Vec<VisualTextRow>,
) -> Vec<VisualTextRow> {
  let mut rows: Vec<VisualTextRow> = rows
    .into_iter()
    .filter_map(|mut row| {
      row.text = normalize_visual_text_row(&row.text);
      if row.text.trim().is_empty() || is_visual_running_header(&row.text) {
        None
      } else {
        Some(row)
      }
    })
    .collect();

  const ISOLATED_GAP: f32 = 30.0;
  while rows.len() >= 2
    && is_digits_only(&rows[0].text)
    && (rows[0].top - rows[1].top).abs() > ISOLATED_GAP
  {
    rows.remove(0);
  }
  while rows.len() >= 2 {
    let last = rows.len() - 1;
    if is_digits_only(&rows[last].text)
      && (rows[last - 1].top - rows[last].top).abs() > ISOLATED_GAP
    {
      rows.remove(last);
    } else {
      break;
    }
  }

  rows
}

pub(crate) fn normalize_visual_text_row(text: &str) -> String {
  let mut normalized = String::with_capacity(text.len());
  for ch in text.chars() {
    if is_private_use_or_format_char(ch) || is_terminal_control_char(ch) {
      continue;
    }
    if ch == '\u{00A0}' {
      normalized.push(' ');
    } else {
      normalized.push(ch);
    }
  }
  normalized
}

fn is_private_use_or_format_char(ch: char) -> bool {
  matches!(
    ch,
    '\u{E000}'..='\u{F8FF}'
      | '\u{F0000}'..='\u{FFFFD}'
      | '\u{100000}'..='\u{10FFFD}'
      | '\u{FEFF}'
      | '\u{200B}'..='\u{200D}'
      | '\u{2060}'
  )
}

/// Cc: ESC and friends are obeyed by a terminal rather than printed, and a
/// glyph's decoded value is document data. See the twin in sanitize/chars.rs
/// for the full reasoning; TAB is kept because it is layout.
fn is_terminal_control_char(ch: char) -> bool {
  ch.is_control() && ch != '\t'
}

fn is_visual_running_header(text: &str) -> bool {
  let trimmed = text.trim();
  if trimmed.is_empty() {
    return false;
  }

  is_chapter_section_visual_header(trimmed)
}

fn is_chapter_section_visual_header(trimmed: &str) -> bool {
  let tokens: Vec<&str> = trimmed.split_whitespace().collect();
  if tokens.len() < 3 || tokens.len() > 6 {
    return false;
  }

  let label = tokens[0];
  if !matches!(label, "CHAPTER" | "SECTION" | "APPENDIX" | "PART") {
    return false;
  }

  let number = tokens[1];
  if number.is_empty() || number.len() > 8 {
    return false;
  }
  if !number.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '.') {
    return false;
  }

  let looks_like_section_id = number.chars().any(|ch| ch.is_ascii_digit())
    || number.chars().all(|ch| ch.is_ascii_uppercase());
  if !looks_like_section_id {
    return false;
  }

  let last = tokens[tokens.len() - 1];
  if last.chars().all(|ch| ch.is_ascii_digit()) {
    return false;
  }

  has_visual_wide_gap_between(trimmed, number, last)
}

fn has_visual_wide_gap_between(trimmed: &str, first: &str, last: &str) -> bool {
  let Some(first_idx) = trimmed.find(first) else {
    return false;
  };
  let first_end = first_idx + first.len();
  let Some(last_start) = trimmed.rfind(last) else {
    return false;
  };
  if last_start <= first_end {
    return false;
  }
  trimmed[first_end..last_start].chars().filter(|ch| *ch == ' ').count() >= 10
}

pub(crate) fn extract_visual_text_rows(
  doc: &pdf_oxide::PdfDocument,
  page_0based: usize,
) -> Option<Vec<VisualTextRow>> {
  let mut lines = doc.extract_text_lines(page_0based).ok()?;
  if lines.is_empty() {
    return None;
  }

  lines.sort_by(|a, b| {
    b.bbox
      .top()
      .partial_cmp(&a.bbox.top())
      .unwrap_or(std::cmp::Ordering::Equal)
      .then_with(|| {
        a.bbox
          .left()
          .partial_cmp(&b.bbox.left())
          .unwrap_or(std::cmp::Ordering::Equal)
      })
  });

  const SAME_ROW_TOL: f32 = 3.0;
  let mut rows = Vec::new();
  let mut row_start = 0usize;
  let mut row_anchor_y = lines[0].bbox.top();
  for i in 1..=lines.len() {
    let break_row = i == lines.len()
      || (row_anchor_y - lines[i].bbox.top()).abs() > SAME_ROW_TOL;
    if break_row {
      let mut row: Vec<&pdf_oxide::layout::TextLine> =
        lines[row_start..i].iter().collect();
      row.sort_by(|a, b| {
        a.bbox
          .left()
          .partial_cmp(&b.bbox.left())
          .unwrap_or(std::cmp::Ordering::Equal)
      });
      let row_left =
        row.iter().map(|l| l.bbox.left()).fold(f32::INFINITY, f32::min);
      let mut body = String::new();
      let mut prev_right: Option<f32> = None;
      for line in row {
        for word in &line.words {
          push_pdf_word_gap(
            &mut body,
            prev_right,
            word.bbox.left(),
            PDF_TEXT_PT_PER_CHAR,
          );
          body.push_str(&word.text);
          prev_right = Some(word.bbox.right());
        }
      }
      rows.push(VisualTextRow {
        top: row_anchor_y,
        left: row_left,
        text: body,
      });
      row_start = i;
      if i < lines.len() {
        row_anchor_y = lines[i].bbox.top();
      }
    }
  }

  Some(rows)
}
