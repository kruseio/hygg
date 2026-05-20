use std::collections::HashSet;

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

fn normalize_extracted_line(line: &str) -> String {
  let mut normalized = String::with_capacity(line.len());
  for ch in line.chars() {
    if is_private_use_or_format_char(ch) {
      continue;
    }
    if ch == '\u{00A0}' {
      normalized.push(' ');
      continue;
    }
    normalized.push(ch);
  }
  normalized
}

fn is_vertical_margin_letter_line(line: &str) -> bool {
  let trimmed_end = line.trim_end();
  if trimmed_end.is_empty() {
    return false;
  }

  let leading_ws =
    trimmed_end.chars().take_while(|ch| ch.is_whitespace()).count();
  if leading_ws < 40 {
    return false;
  }

  let content = trimmed_end.trim_start();
  let mut chars = content.chars();
  let Some(ch) = chars.next() else {
    return false;
  };

  chars.next().is_none() && ch.is_alphabetic()
}

fn has_wide_gap_before_page_number(trimmed: &str) -> bool {
  let Some(last_token) = trimmed.split_whitespace().last() else {
    return false;
  };
  if !last_token.chars().all(|ch| ch.is_ascii_digit()) {
    return false;
  }

  let Some(number_start) = trimmed.rfind(last_token) else {
    return false;
  };
  let before_number = &trimmed[..number_start];
  let gap =
    before_number.chars().rev().take_while(|ch| ch.is_whitespace()).count();

  gap >= 10
}

fn is_running_header_or_footer_line(line: &str) -> bool {
  let trimmed = line.trim();
  if trimmed.is_empty() {
    return false;
  }

  let leading_ws = line.chars().take_while(|ch| ch.is_whitespace()).count();
  let is_page_number_only = trimmed.chars().all(|ch| ch.is_ascii_digit());
  if is_page_number_only && leading_ws >= 20 {
    return true;
  }

  let trailing_page = trimmed
    .split_whitespace()
    .last()
    .is_some_and(|token| token.chars().all(|ch| ch.is_ascii_digit()));
  if !trailing_page {
    return false;
  }

  if (trimmed.contains("CHAPTER") || trimmed.contains("SECTION"))
    && leading_ws >= 20
  {
    return true;
  }

  if has_wide_gap_before_page_number(trimmed) {
    let before_number = trimmed
      .split_whitespace()
      .collect::<Vec<_>>()
      .split_last()
      .map(|(_, rest)| rest.join(" "))
      .unwrap_or_default();
    if before_number.split_whitespace().count() <= 6 {
      return true;
    }
  }

  false
}

fn centered_heading_label(line: &str) -> Option<&str> {
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

pub(crate) fn sanitize_layout_text(text: &str) -> String {
  let mut output = String::with_capacity(text.len());
  let mut blank_run = 0usize;
  let mut seen_centered_headings: HashSet<String> = HashSet::new();

  for raw_line in text.lines() {
    let line = normalize_extracted_line(raw_line);
    if is_vertical_margin_letter_line(&line)
      || is_running_header_or_footer_line(&line)
    {
      continue;
    }

    if let Some(label) = centered_heading_label(&line) {
      if seen_centered_headings.contains(label) {
        continue;
      }
      seen_centered_headings.insert(label.to_string());
    }

    if line.trim().is_empty() {
      blank_run += 1;
      if blank_run > 3 {
        continue;
      }
    } else {
      blank_run = 0;
    }

    output.push_str(&line);
    output.push('\n');
  }

  output
}

#[cfg(test)]
mod tests {
  use super::{
    centered_heading_label, is_running_header_or_footer_line,
    normalize_extracted_line, sanitize_layout_text,
  };

  #[test]
  fn removes_vertical_margin_letter_lines_and_excess_blank_lines() {
    let input = concat!(
      "Contents 8\n",
      "                                                                                                   C\n",
      "                                                                                                   o\n",
      "                                                                                                   n\n",
      "\n",
      "\n",
      "\n",
      "\n",
      "Body\n"
    );

    let output = sanitize_layout_text(input);
    assert!(!output.contains("\n                                                                                                   C\n"));
    assert!(!output.contains("\n                                                                                                   o\n"));
    assert!(!output.contains("\n                                                                                                   n\n"));
    assert!(!output.contains("\n\n\n\n\n"));
    assert!(output.contains("Contents 8"));
    assert!(output.contains("Body"));
  }

  #[test]
  fn keeps_normal_single_letter_lines() {
    let input = "A\n  B\nShort line\n";
    let output = sanitize_layout_text(input);

    assert!(output.contains("\nA\n") || output.starts_with("A\n"));
    assert!(output.contains("\n  B\n") || output.starts_with("  B\n"));
    assert!(output.contains("Short line\n"));
  }

  #[test]
  fn removes_running_header_and_footer_lines() {
    let input = concat!(
      "                                                                                                           IntroductionCHAPTER 1                                         28\n",
      "  Preface                                                 24\n",
      "Body paragraph line\n"
    );

    let output = sanitize_layout_text(input);
    assert!(!output.contains("IntroductionCHAPTER 1"));
    assert!(
      !output
        .contains("Preface                                                 24")
    );
    assert!(output.contains("Body paragraph line"));
  }

  #[test]
  fn keeps_regular_toc_rows_with_page_numbers() {
    let line = "  4.16       Starting a new triangle in a free-form Gouraud-shaded triangle mesh   316";
    assert!(
      !is_running_header_or_footer_line(line),
      "expected TOC row to stay, got: {line}"
    );
  }

  #[test]
  fn removes_duplicate_centered_heading_lines() {
    let input = concat!(
      "                Figures\n",
      "  9.9         Rendering of the 3D artwork using View0 (no cross section)   824\n",
      "                                                                                                           Figures\n",
      "  9.10       Rendering of the 3D artwork using View1 (cross section perpendicular to the \n"
    );

    let output = sanitize_layout_text(input);
    assert_eq!(
      output.matches("Figures").count(),
      1,
      "expected duplicate centered heading to be removed, got: {output:?}"
    );
  }

  #[test]
  fn detects_supported_centered_heading_labels() {
    assert_eq!(
      centered_heading_label("                Figures"),
      Some("Figures")
    );
    assert_eq!(
      centered_heading_label("                Contents"),
      Some("Contents")
    );
    assert_eq!(centered_heading_label("Body heading"), None);
  }

  #[test]
  fn removes_private_use_icon_only_lines() {
    let input = concat!("Before\n", "  \u{f05a}\n", "After\n",);

    let output = sanitize_layout_text(input);
    assert!(
      !output.contains('\u{f05a}'),
      "expected private-use icon to be removed, got: {output:?}"
    );
    assert!(output.contains("Before"));
    assert!(output.contains("After"));
  }

  #[test]
  fn removes_private_use_icons_from_inline_callouts() {
    let input = "  \u{f0eb}        Helpful tip text\n";
    let normalized = normalize_extracted_line(input);
    assert!(
      !normalized.contains('\u{f0eb}'),
      "expected inline private-use icon to be removed, got: {normalized:?}"
    );
    assert!(
      normalized.contains("Helpful tip text"),
      "expected remaining callout text to be preserved, got: {normalized:?}"
    );
  }

  #[test]
  fn normalizes_nbsp_to_ascii_space() {
    let input = "A\u{00a0}B\n";
    let output = sanitize_layout_text(input);
    assert!(
      output.contains("A B"),
      "expected nbsp to normalize to plain space, got: {output:?}"
    );
  }
}
