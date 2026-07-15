use super::labels::{
  LabelKind, classify_label, is_cluster_boundary_line,
  is_figure_or_table_caption, leading_ws_len, next_non_blank, prev_non_blank,
};

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
pub(crate) fn strip_diagram_labels(text: &str) -> String {
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
