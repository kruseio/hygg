use crate::heuristics::{
  is_code_like_line, is_heading_like_line, is_intro_line,
};
use std::collections::HashSet;

fn is_text_show_operator(operator: &str) -> bool {
  matches!(operator, "Tj" | "TJ" | "'" | "\"")
}

fn decode_ascii_pdf_text(bytes: &[u8]) -> Option<String> {
  if bytes.is_empty() {
    return None;
  }

  let text = String::from_utf8_lossy(bytes);
  let char_count = text.chars().count();
  if char_count == 0 {
    return None;
  }

  let printable_count = text
    .chars()
    .filter(|ch| ch.is_ascii_graphic() || ch.is_ascii_whitespace())
    .count();
  if printable_count.saturating_mul(100) < char_count.saturating_mul(95) {
    return None;
  }

  let trimmed = text.trim();
  if trimmed.is_empty() {
    return None;
  }

  Some(trimmed.to_string())
}

fn intro_line_likely_example(trimmed: &str) -> bool {
  let lower = trimmed.to_ascii_lowercase();
  ["example", "sample", "template", "config", "configuration", "ignore", "file"]
    .iter()
    .any(|keyword| lower.contains(keyword))
}

fn looks_like_path_pattern_line(line: &str) -> bool {
  line.contains('*') || line.contains('/') || line.starts_with('!')
}

fn sparse_block_anchor_indices(lines: &[String]) -> Vec<usize> {
  let mut anchors = Vec::new();

  for (idx, line) in lines.iter().enumerate() {
    let trimmed = line.trim();
    if !is_intro_line(trimmed) || !intro_line_likely_example(trimmed) {
      continue;
    }

    let mut code_lines = 0usize;
    let mut first_code_line = None;
    let mut saw_heading = false;
    for (next_idx, next_line) in lines.iter().enumerate().skip(idx + 1) {
      let next_trimmed = next_line.trim();
      if next_trimmed.is_empty() {
        continue;
      }
      if is_heading_like_line(next_trimmed) {
        saw_heading = true;
        break;
      }
      if is_code_like_line(next_trimmed) {
        code_lines += 1;
        first_code_line.get_or_insert(next_idx);
        if code_lines > 1 {
          break;
        }
      }
    }

    if !saw_heading || code_lines > 1 {
      continue;
    }

    let Some(anchor) = first_code_line else {
      continue;
    };
    let anchor_text = lines[anchor].trim();
    if anchor_text.starts_with('#') {
      anchors.push(anchor);
    }
  }

  anchors
}

pub(crate) fn extract_ascii_pdf_text_chunks(
  canonical_path: &std::path::Path,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
  use lopdf::Object;
  use lopdf::content::Content;

  let doc = lopdf::Document::load(canonical_path)?;
  let mut chunks = Vec::new();

  for (_, page_id) in doc.get_pages() {
    let content_data = doc.get_page_content(page_id)?;
    let content = Content::decode(&content_data)?;

    for op in content.operations {
      if !is_text_show_operator(&op.operator) {
        continue;
      }

      for operand in op.operands {
        match operand {
          Object::String(bytes, _) => {
            if let Some(text) = decode_ascii_pdf_text(&bytes) {
              chunks.push(text);
            }
          }
          Object::Array(items) => {
            for item in items {
              if let Object::String(bytes, _) = item
                && let Some(text) = decode_ascii_pdf_text(&bytes)
              {
                chunks.push(text);
              }
            }
          }
          _ => {}
        }
      }
    }
  }

  Ok(chunks)
}

fn recovered_code_lines(
  chunks: &[String],
  anchor: &str,
  existing: &HashSet<String>,
) -> Vec<String> {
  let Some(start) = chunks.iter().position(|chunk| chunk.trim() == anchor)
  else {
    return Vec::new();
  };

  let mut recovered = Vec::new();
  let mut non_code_streak = 0usize;

  for chunk in chunks.iter().skip(start + 1).take(80) {
    let trimmed = chunk.trim();
    if trimmed.is_empty() || trimmed == "!" {
      continue;
    }
    if is_heading_like_line(trimmed) {
      break;
    }
    if is_code_like_line(trimmed) {
      non_code_streak = 0;
      if !existing.contains(trimmed) {
        recovered.push(trimmed.to_string());
      }
      continue;
    }

    non_code_streak += 1;
    if non_code_streak >= 2 {
      break;
    }
  }

  recovered
}

fn recovery_is_confident(recovered: &[String]) -> bool {
  recovered.len() >= 4
    && recovered
      .iter()
      .filter(|line| looks_like_path_pattern_line(line))
      .count()
      >= 2
}

pub(crate) fn recover_sparse_code_blocks(
  canonical_path: &std::path::Path,
  extracted_text: &str,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
  let mut lines: Vec<String> =
    extracted_text.lines().map(str::to_string).collect();
  let anchors = sparse_block_anchor_indices(&lines);
  if anchors.is_empty() {
    return Ok(None);
  }

  let chunks = extract_ascii_pdf_text_chunks(canonical_path)?;
  let mut existing: HashSet<String> =
    lines.iter().map(|line| line.trim().to_string()).collect();
  let mut changed = false;

  for anchor_idx in anchors.into_iter().rev() {
    let Some(anchor_line) = lines.get(anchor_idx) else {
      continue;
    };
    let anchor = anchor_line.trim();
    if anchor.is_empty() {
      continue;
    }

    let recovered = recovered_code_lines(&chunks, anchor, &existing);
    if !recovery_is_confident(&recovered) {
      continue;
    }

    let indent: String =
      anchor_line.chars().take_while(|ch| ch.is_whitespace()).collect();
    let mut insertions = Vec::new();
    for line in recovered {
      existing.insert(line.clone());
      if indent.is_empty() {
        insertions.push(line);
      } else {
        insertions.push(format!("{indent}{line}"));
      }
    }

    lines.splice(anchor_idx + 1..anchor_idx + 1, insertions);
    changed = true;
  }

  if !changed {
    return Ok(None);
  }

  let mut rebuilt = lines.join("\n");
  if extracted_text.ends_with('\n') {
    rebuilt.push('\n');
  }
  Ok(Some(rebuilt))
}
