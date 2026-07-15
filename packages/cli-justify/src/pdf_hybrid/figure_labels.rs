use crate::text_utils::{is_ascii_numeric, leading_whitespace_width};

fn looks_like_sparse_figure_label_line(line: &str) -> bool {
  let trimmed = line.trim();
  if trimmed.is_empty() || trimmed.starts_with("FIGURE ") {
    return false;
  }

  let leading_ws = leading_whitespace_width(line);
  if leading_ws < 20 {
    return false;
  }

  let words: Vec<&str> = trimmed.split_whitespace().collect();
  if words.is_empty() || words.len() > 6 {
    return false;
  }
  if words.last().is_some_and(|token| is_ascii_numeric(token)) {
    return false;
  }

  trimmed.chars().any(|ch| ch.is_alphabetic())
    && !trimmed.ends_with(['.', ',', ';', ':'])
}

pub(super) fn normalize_sparse_figure_label_blocks(
  lines: &mut [String],
  line_width: usize,
) {
  let mut idx = 0usize;
  while idx < lines.len() {
    if !looks_like_sparse_figure_label_line(&lines[idx]) {
      idx += 1;
      continue;
    }

    let block_start = idx;
    while idx < lines.len() && looks_like_sparse_figure_label_line(&lines[idx])
    {
      idx += 1;
    }
    let block_end = idx;
    let block_len = block_end - block_start;
    if block_len < 3 {
      continue;
    }

    let mut centers = Vec::with_capacity(block_len);
    for line in &lines[block_start..block_end] {
      let leading_ws = leading_whitespace_width(line);
      let text = line.trim_start_matches([' ', '\t']);
      centers.push(leading_ws + text.chars().count() / 2);
    }
    if centers.is_empty() {
      continue;
    }

    let avg_center =
      centers.iter().copied().sum::<usize>() / centers.len().max(1);
    let target_center = line_width / 2;
    if avg_center <= target_center + 4 {
      continue;
    }
    let shift = avg_center - target_center;

    for line in &mut lines[block_start..block_end] {
      let text = line.trim_start_matches([' ', '\t']);
      let text_len = text.chars().count();
      let old_indent = leading_whitespace_width(line);
      let max_indent = line_width.saturating_sub(text_len);
      let new_indent = old_indent.saturating_sub(shift).min(max_indent);
      *line = format!("{}{}", " ".repeat(new_indent), text);
    }
  }
}
