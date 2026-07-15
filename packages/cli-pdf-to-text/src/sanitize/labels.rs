use crate::heuristics::is_code_like_line;

pub(crate) fn centered_heading_label(line: &str) -> Option<&str> {
  let trimmed = line.trim();
  if trimmed.is_empty() {
    return None;
  }

  let leading_ws = line.chars().take_while(|ch| ch.is_whitespace()).count();
  if leading_ws < 12 {
    return None;
  }

  let words: Vec<&str> = trimmed.split_whitespace().collect();
  if words.len() != 1 {
    return None;
  }

  match words[0] {
    "Contents" | "Figures" | "Tables" => Some(words[0]),
    _ => None,
  }
}

pub(crate) fn is_section_number_token(token: &str) -> bool {
  let mut has_digit = false;
  for ch in token.chars() {
    if ch.is_ascii_digit() {
      has_digit = true;
    } else if ch != '.' {
      return false;
    }
  }
  has_digit
}

pub(crate) fn is_figure_or_table_caption(trimmed: &str) -> bool {
  trimmed.starts_with("FIGURE ")
    || trimmed.starts_with("Figure ")
    || trimmed.starts_with("TABLE ")
    || trimmed.starts_with("Table ")
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum LabelKind {
  Strong,
  Weak,
}

pub(crate) fn leading_ws_len(line: &str) -> usize {
  line.chars().take_while(|ch| ch.is_whitespace()).count()
}

pub(crate) fn classify_label(line: &str) -> Option<LabelKind> {
  let trimmed = line.trim();
  if trimmed.is_empty() {
    return None;
  }

  // Captions are handled separately, never treated as labels.
  if is_figure_or_table_caption(trimmed) {
    return None;
  }

  let words: Vec<&str> = trimmed.split_whitespace().collect();
  if words.is_empty() || words.len() > 5 {
    return None;
  }

  // Section heading numbers ("2.4 ...", "G.1 ...").
  if let Some(first) = words.first()
    && is_section_number_token(first)
  {
    return None;
  }

  // TOC / footer-like lines that end with a page number.
  if let Some(last) = words.last()
    && last.chars().all(|c| c.is_ascii_digit())
  {
    return None;
  }

  // Sentence-terminator => probably prose, not a label.
  if trimmed.ends_with(['.', ',', ':', ';', '!', '?']) {
    return None;
  }

  // Must contain at least one letter.
  if !trimmed.chars().any(|c| c.is_alphabetic()) {
    return None;
  }

  // Bullets / list markers.
  let first_char = trimmed.chars().next();
  if matches!(first_char, Some('•' | '·' | '◦' | '▪' | '▫' | '◆' | '►'))
  {
    return None;
  }

  if is_code_like_line(trimmed) {
    Some(LabelKind::Weak)
  } else {
    Some(LabelKind::Strong)
  }
}

pub(crate) fn is_cluster_boundary_line(line: &str) -> bool {
  let trimmed = line.trim();
  // Any non-blank line that does not classify as a label candidate counts
  // as a boundary: paragraphs, section headings, list bullets, headers,
  // and so on. Captions cannot appear here because the cluster builder
  // greedily consumes them.
  !trimmed.is_empty() && classify_label(line).is_none()
}

pub(crate) fn prev_non_blank(lines: &[&str], start: usize) -> Option<usize> {
  let mut idx = start;
  while idx > 0 {
    idx -= 1;
    if !lines[idx].trim().is_empty() {
      return Some(idx);
    }
  }
  None
}

pub(crate) fn next_non_blank(lines: &[&str], start: usize) -> Option<usize> {
  let mut idx = start;
  while idx < lines.len() {
    if !lines[idx].trim().is_empty() {
      return Some(idx);
    }
    idx += 1;
  }
  None
}
