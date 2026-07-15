//! Map extracted [`crate::PdfVisual`]s onto a client's flattened lines, without
//! touching that model. Shared by every rich client (native GUI, browser PWA)
//! so figure/table placement is computed one way everywhere.
//!
//! Figures line up by reading order — the Nth ASCII-art block on a page is the
//! Nth extracted image. Tables line up by matching their cell text against the
//! page's flattened text lines. Anything that doesn't line up confidently is
//! dropped, so the worst case is a figure/table that stays as text/ASCII. None
//! of this changes the lines, kinds, or anchors, so reading progress is
//! unaffected and stays identical across clients.

use std::collections::{BTreeMap, HashSet};

use crate::{PdfVisual, PdfVisualKind};

/// A resolved placement: `visual` (an index into the input slice) covers the
/// line range `[line_start, line_start + line_count)`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VisualPlacement {
  pub visual: usize,
  pub line_start: usize,
  pub line_count: usize,
}

/// Correlate visuals to line ranges. `is_image(i)` reports whether flattened
/// line `i` is an ASCII-art (image) row — the client's own `LineKind` check.
/// Returns placements sorted by `line_start`, non-overlapping.
pub fn place_visuals(
  visuals: &[PdfVisual],
  lines: &[String],
  is_image: impl Fn(usize) -> bool,
  page_starts: &[usize],
) -> Vec<VisualPlacement> {
  let mut occupied = vec![false; lines.len()];
  let mut out: Vec<VisualPlacement> = Vec::new();

  // Images: the Nth ASCII-art block on a page is the Nth extracted image (both
  // top-down). Only map a page when the counts line up exactly.
  let mut by_page: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
  for (i, v) in visuals.iter().enumerate() {
    if v.kind == PdfVisualKind::Image {
      by_page.entry(v.page).or_default().push(i);
    }
  }
  for (page, mut idxs) in by_page {
    let Some((start, end)) = page_range(page_starts, lines.len(), page) else {
      continue;
    };
    let blocks = image_blocks(lines, &is_image, start, end);
    idxs.sort_by_key(|&i| visuals[i].ordinal);
    if blocks.len() != idxs.len() {
      continue;
    }
    for (&(bstart, blen), &vi) in blocks.iter().zip(idxs.iter()) {
      occupied[bstart..bstart + blen].fill(true);
      out.push(VisualPlacement {
        visual: vi,
        line_start: bstart,
        line_count: blen,
      });
    }
  }

  // Tables: locate each table's rows among the page's flattened text lines.
  let mut tables: Vec<usize> = visuals
    .iter()
    .enumerate()
    .filter(|(_, v)| v.kind == PdfVisualKind::Table)
    .map(|(i, _)| i)
    .collect();
  tables.sort_by_key(|&i| (visuals[i].page, visuals[i].ordinal));
  for vi in tables {
    let v = &visuals[vi];
    let (Some(text), Some((start, end))) =
      (v.text.as_deref(), page_range(page_starts, lines.len(), v.page))
    else {
      continue;
    };
    if let Some((ls, lc)) =
      match_table(lines, &is_image, &occupied, start, end, text)
    {
      occupied[ls..ls + lc].fill(true);
      out.push(VisualPlacement { visual: vi, line_start: ls, line_count: lc });
    }
  }

  out.sort_by_key(|p| p.line_start);
  out
}

/// The `[start, end)` flattened-line range of a 1-based page.
fn page_range(
  page_starts: &[usize],
  lines_len: usize,
  page_1based: usize,
) -> Option<(usize, usize)> {
  let idx = page_1based.checked_sub(1)?;
  let start = *page_starts.get(idx)?;
  let end = page_starts.get(idx + 1).copied().unwrap_or(lines_len);
  (start < end).then_some((start, end))
}

/// Contiguous runs of ASCII-art lines within `[start, end)`, split where the
/// left indent changes (a new figure at a different x).
fn image_blocks(
  lines: &[String],
  is_image: &impl Fn(usize) -> bool,
  start: usize,
  end: usize,
) -> Vec<(usize, usize)> {
  let mut blocks = Vec::new();
  let mut i = start;
  while i < end {
    if !is_image(i) {
      i += 1;
      continue;
    }
    let bstart = i;
    let indent = leading_spaces(&lines[i]);
    i += 1;
    while i < end && is_image(i) && leading_spaces(&lines[i]) == indent {
      i += 1;
    }
    blocks.push((bstart, i - bstart));
  }
  blocks
}

/// Find the contiguous run of text lines in `[start, end)` that best matches a
/// table's cell text. Conservative: returns `None` unless a real, well-covered
/// multi-line run is found, so a false match never hides prose.
fn match_table(
  lines: &[String],
  is_image: &impl Fn(usize) -> bool,
  occupied: &[bool],
  start: usize,
  end: usize,
  table_text: &str,
) -> Option<(usize, usize)> {
  let tset: HashSet<&str> =
    table_text.split_whitespace().filter(|t| t.len() >= 2).collect();
  if tset.len() < 3 {
    return None;
  }
  let belongs = |i: usize| -> bool {
    if occupied[i] || is_image(i) {
      return false;
    }
    let toks = tokens(&lines[i]);
    !toks.is_empty()
      && toks.iter().filter(|t| tset.contains(t.as_str())).count() * 5
        >= toks.len() * 3
  };

  let mut best: Option<(usize, usize, usize)> = None; // (start, len, covered)
  let mut i = start;
  while i < end {
    if !belongs(i) {
      i += 1;
      continue;
    }
    let ls = i;
    let mut covered: HashSet<&str> = HashSet::new();
    while i < end && belongs(i) {
      for t in tokens(&lines[i]) {
        if let Some(&tok) = tset.get(t.as_str()) {
          covered.insert(tok);
        }
      }
      i += 1;
    }
    let run = (ls, i - ls, covered.len());
    if best.is_none_or(|b| run.2 > b.2) {
      best = Some(run);
    }
  }

  let (ls, lc, covered) = best?;
  (lc >= 2 && covered * 2 >= tset.len()).then_some((ls, lc))
}

fn leading_spaces(s: &str) -> usize {
  s.chars().take_while(|c| *c == ' ').count()
}

fn tokens(s: &str) -> Vec<String> {
  s.split_whitespace()
    .map(|w| w.to_lowercase())
    .filter(|w| w.len() >= 2)
    .collect()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn img_at(idxs: &[usize]) -> impl Fn(usize) -> bool + '_ {
    move |i| idxs.contains(&i)
  }

  #[test]
  fn groups_image_blocks_by_indent() {
    let lines: Vec<String> =
      ["  a", "  b", "c", "    d", "    e"].map(String::from).to_vec();
    // Lines 0,1,3,4 are ASCII-art; line 2 is text.
    let is_img = img_at(&[0, 1, 3, 4]);
    // Two blocks: [0,1] (indent 2) and [3,4] (indent 4).
    assert_eq!(image_blocks(&lines, &is_img, 0, 5), vec![(0, 2), (3, 2)]);
  }

  #[test]
  fn matches_a_table_run_and_ignores_prose() {
    let lines: Vec<String> = [
      "Some intro prose here about nothing",
      "alpha beta 10",
      "gamma delta 20",
      "epsilon zeta 30",
      "More unrelated closing prose",
    ]
    .map(String::from)
    .to_vec();
    let no_img = img_at(&[]);
    let occ = vec![false; lines.len()];
    let text = "alpha beta 10 gamma delta 20 epsilon zeta 30";
    assert_eq!(match_table(&lines, &no_img, &occ, 0, 5, text), Some((1, 3)));
    // A single matching line is too short / too weakly covered → no match.
    assert_eq!(match_table(&lines, &no_img, &occ, 1, 2, text), None);
  }
}
