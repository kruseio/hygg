use crate::heuristics::is_code_like_line;
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

fn is_section_number_token(token: &str) -> bool {
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

fn is_figure_or_table_caption(trimmed: &str) -> bool {
  trimmed.starts_with("FIGURE ")
    || trimmed.starts_with("Figure ")
    || trimmed.starts_with("TABLE ")
    || trimmed.starts_with("Table ")
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LabelKind {
  Strong,
  Weak,
}

fn leading_ws_len(line: &str) -> usize {
  line.chars().take_while(|ch| ch.is_whitespace()).count()
}

fn classify_label(line: &str) -> Option<LabelKind> {
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

fn is_cluster_boundary_line(line: &str) -> bool {
  let trimmed = line.trim();
  // Any non-blank line that does not classify as a label candidate counts
  // as a boundary: paragraphs, section headings, list bullets, headers,
  // and so on. Captions cannot appear here because the cluster builder
  // greedily consumes them.
  !trimmed.is_empty() && classify_label(line).is_none()
}

fn prev_non_blank(lines: &[&str], start: usize) -> Option<usize> {
  let mut idx = start;
  while idx > 0 {
    idx -= 1;
    if !lines[idx].trim().is_empty() {
      return Some(idx);
    }
  }
  None
}

fn next_non_blank(lines: &[&str], start: usize) -> Option<usize> {
  let mut idx = start;
  while idx < lines.len() {
    if !lines[idx].trim().is_empty() {
      return Some(idx);
    }
    idx += 1;
  }
  None
}

/// Strip clusters of short, irregularly-indented lines that come from
/// diagram-internal labels (boxes/arrows annotations in figures).
///
/// A cluster is built starting at a strong label and extends through
/// consecutive labels, blank lines (up to 2 in a row), and FIGURE/TABLE
/// captions. The cluster is dropped (captions kept) only when:
///   * it has enough strong labels with varied indentation, AND
///   * it is sandwiched between body-text-like lines on both sides (so we don't
///     shred title pages, dedication pages, or other legitimately sparse
///     top/bottom-of-document content).
fn strip_diagram_labels(text: &str) -> String {
  let lines: Vec<&str> = text.lines().collect();
  let mut drop = vec![false; lines.len()];
  let mut i = 0;

  while i < lines.len() {
    if !matches!(classify_label(lines[i]), Some(LabelKind::Strong)) {
      i += 1;
      continue;
    }

    let mut label_indices: Vec<usize> = Vec::new();
    let mut strong_count = 0usize;
    let mut has_caption = false;
    let mut indents: Vec<usize> = Vec::new();
    let mut j = i;

    loop {
      if j >= lines.len() {
        break;
      }
      let trimmed = lines[j].trim();

      if trimmed.is_empty() {
        let mut k = j + 1;
        while k < lines.len() && lines[k].trim().is_empty() {
          k += 1;
        }
        // Tolerate up to 2 consecutive blank lines inside a cluster.
        if k - j > 2 || k >= lines.len() {
          break;
        }
        let next_trimmed = lines[k].trim();
        if is_figure_or_table_caption(next_trimmed)
          || classify_label(lines[k]).is_some()
        {
          j = k;
          continue;
        }
        break;
      }

      if is_figure_or_table_caption(trimmed) {
        has_caption = true;
        j += 1;
        continue;
      }

      match classify_label(lines[j]) {
        Some(kind) => {
          if matches!(kind, LabelKind::Strong) {
            strong_count += 1;
          }
          label_indices.push(j);
          indents.push(leading_ws_len(lines[j]));
          j += 1;
        }
        None => break,
      }
    }

    indents.sort_unstable();
    indents.dedup();
    let distinct_indents = indents.len();

    let bounded_above = prev_non_blank(&lines, i)
      .is_some_and(|idx| is_cluster_boundary_line(lines[idx]));
    let bounded_below = next_non_blank(&lines, j)
      .is_some_and(|idx| is_cluster_boundary_line(lines[idx]));

    let label_shape_ok = (strong_count >= 3 && distinct_indents >= 3)
      || (strong_count >= 2 && has_caption && distinct_indents >= 2);

    if label_shape_ok && bounded_above && bounded_below {
      for idx in label_indices {
        drop[idx] = true;
      }
    }

    i = j.max(i + 1);
  }

  let mut out = String::with_capacity(text.len());
  for (idx, line) in lines.iter().enumerate() {
    if drop[idx] {
      continue;
    }
    out.push_str(line);
    out.push('\n');
  }
  out
}

pub(crate) fn sanitize_layout_text(text: &str) -> String {
  let text = strip_diagram_labels(text);
  let text = text.as_str();
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
    } else {
      // Also drop later un-centered occurrences of a label we've already
      // seen as a centered heading. The positional extractor can land a
      // running header at column 0 on facing pages (the verso margin sits
      // left of the recto body), so the leading-whitespace check above
      // misses it — but the literal text still matches the title we
      // already kept, and the duplicate would otherwise leak through.
      let trimmed = line.trim();
      if seen_centered_headings.contains(trimmed) {
        continue;
      }
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
    normalize_extracted_line, sanitize_layout_text, strip_diagram_labels,
  };

  #[test]
  fn strips_figure_label_cluster_above_caption() {
    let input = concat!(
      "         viewer application, such as Acrobat, on any supported platform.\n",
      "                                            Acrobat\n",
      "Macintosh application Windows application\n",
      "                                          Adobe PDF\n",
      "                                             printer\n",
      "\n",
      "  QuickDraw/\n",
      "\n",
      "         CoreGraphics\n",
      "                                                     GDI\n",
      "                                                   PDF\n",
      "\n",
      "                         FIGURE 2.2   Creating PDF files using Acrobat Distiller\n",
      "  2.4 PDF and the PostScript Language\n",
      "         The PDF operators for setting the graphics state and painting graphics\n",
    );

    let output = strip_diagram_labels(input);
    let body = "         viewer application, such as Acrobat, on any supported platform.";
    let caption = "                         FIGURE 2.2   Creating PDF files using Acrobat Distiller";
    let next_section = "  2.4 PDF and the PostScript Language";
    let para = "         The PDF operators for setting the graphics state and painting graphics";

    assert!(output.contains(body), "body paragraph should survive: {output:?}");
    assert!(
      output.contains(caption),
      "FIGURE caption should survive: {output:?}"
    );
    assert!(
      output.contains(next_section),
      "section heading should survive: {output:?}"
    );
    assert!(
      output.contains(para),
      "following paragraph should survive: {output:?}"
    );

    for label in [
      "Acrobat\n",
      "Macintosh application Windows application",
      "Adobe PDF\n",
      "printer\n",
      "CoreGraphics\n",
      "GDI\n",
      "PDF\n\n",
    ] {
      assert!(
        !output.contains(label),
        "expected figure label {label:?} to be stripped, got:\n{output}"
      );
    }
  }

  #[test]
  fn strips_unattributed_figure_label_cluster_mid_paragraph() {
    let input = concat!(
      "         (although a few such devices do also\n",
      "                                    PostScript\n",
      "                                page description\n",
      "                                     Acrobat\n",
      "                                        PDF\n",
      "                               Acrobat Distiller\n",
      "\n",
      "         support  PDF  directly).  An  application  printing a PDF document to a\n",
    );

    let output = strip_diagram_labels(input);
    assert!(output.contains("(although a few such devices do also"));
    assert!(output.contains("support  PDF  directly)"));
    for label in
      ["PostScript\n", "page description", "Acrobat\n", "Acrobat Distiller"]
    {
      assert!(
        !output.contains(label),
        "expected {label:?} stripped:\n{output}"
      );
    }
  }

  #[test]
  fn preserves_title_page_without_paragraph_above() {
    // Title page is a cluster of labels but has no body paragraph above it,
    // so the heuristic must not strip it.
    let input = concat!(
      "PDF Reference\n",
      "   sixth edition\n",
      "   Adobe® Portable Document Format\n",
      "         Version 1.7\n",
      "        Adobe Systems Incorporated\n",
      "\n",
      "© 1985–2006 Adobe® Systems Incorporated. All rights reserved.\n",
    );

    let output = strip_diagram_labels(input);
    assert!(output.contains("PDF Reference"));
    assert!(output.contains("sixth edition"));
    assert!(output.contains("Adobe® Portable Document Format"));
    assert!(output.contains("Version 1.7"));
    assert!(output.contains("Adobe Systems Incorporated"));
  }

  #[test]
  fn preserves_uniformly_indented_short_list() {
    // A short vertical list at one indent level is not a diagram. The
    // distinct-indents requirement keeps such lists intact.
    let input = concat!(
      "The supported commands are listed below.\n",
      "  cat\n",
      "  ls\n",
      "  cp\n",
      "  mv\n",
      "These commands operate on files.\n",
    );

    let output = strip_diagram_labels(input);
    assert!(output.contains("cat"));
    assert!(output.contains("ls"));
    assert!(output.contains("cp"));
    assert!(output.contains("mv"));
  }

  #[test]
  fn preserves_code_block_recovery_anchor() {
    // The .gitignore example used by stream_recovery looks like a cluster
    // of weak (code-like) labels. With zero strong labels, the cluster must
    // not be stripped or recovery will have no anchor.
    let input = concat!(
      "Here is another example .gitignore file:\n",
      "  *.a\n",
      "  !lib.a\n",
      "  /TODO\n",
      "  build/\n",
      "  doc/*.txt\n",
      "  doc/**/*.pdf\n",
      "More body text follows.\n",
    );

    let output = strip_diagram_labels(input);
    for line in
      ["*.a", "!lib.a", "/TODO", "build/", "doc/*.txt", "doc/**/*.pdf"]
    {
      assert!(output.contains(line), "code line {line:?} should survive");
    }
  }

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
