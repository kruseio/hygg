mod chars;
mod diagram;
mod headers;
mod labels;

#[cfg(test)]
mod tests;

use std::collections::HashSet;

use self::chars::normalize_extracted_line;
use self::diagram::strip_diagram_labels;
use self::headers::{
  is_running_header_or_footer_line, is_vertical_margin_letter_line,
};
use self::labels::centered_heading_label;

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
