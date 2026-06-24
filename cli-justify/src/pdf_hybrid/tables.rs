//! Multi-column table reflow for the PDF formatter.
//!
//! The layout text extractor turns a PDF table into rows whose columns are
//! separated by runs of spaces proportional to the gap between the source
//! glyphs. When such a row is wider than the terminal, the generic
//! whitespace-preserving wrapper word-wraps it and dumps the overflow columns
//! onto a short-indented continuation line — the "smushed" look reported in
//! issue #92.
//!
//! This module instead treats a run of consecutive multi-column rows as one
//! table: it recovers the column grid from the cell start positions, then
//! re-renders the block so every row keeps its cells on a single logical row
//! with the columns vertically aligned and compressed (and, only when
//! unavoidable, wrapped) to fit the width.

use crate::text_utils::char_len;

/// A run of spaces this wide (or wider) inside a row is read as a column
/// separator when *deciding whether a line is a table row*. Kept high so prose
/// with the occasional double space is never mistaken for a table.
const COLUMN_GAP_SIGNAL: usize = 5;

/// A run of spaces this wide (or wider) splits two cells when *tokenising* a
/// known table row. Lower than the detection threshold so a value like
/// `1 (9)` keeps its single internal space while still separating columns.
const CELL_SPLIT_GAP: usize = 2;

/// Preferred number of spaces between rendered columns; dropped to 1 before
/// any column content is wrapped.
const PREFERRED_GAP: usize = 2;
const MIN_GAP: usize = 1;

/// A cell parsed out of a row: where it started (in characters from the line
/// start) and its trimmed text.
struct Cell {
  start: usize,
  text: String,
}

/// Count runs of `threshold`-or-wider spaces *between* non-space characters
/// (leading indentation is ignored).
fn count_wide_gaps(line: &str, threshold: usize) -> usize {
  let mut count = 0usize;
  let mut run = 0usize;
  let mut seen_non_space = false;
  for ch in line.chars() {
    if ch == ' ' {
      if seen_non_space {
        run += 1;
      }
      continue;
    }
    if run >= threshold {
      count += 1;
    }
    run = 0;
    seen_non_space = true;
  }
  count
}

/// True for a genuine multi-column grid row: at least three space-separated
/// words and at least two wide internal gaps (i.e. three or more columns).
///
/// The two-wide-gap requirement is what keeps this from colliding with the
/// formatter's other preserved-layout shapes — two-column option/spec tables,
/// `Figure N`/`Plate N` caption listings and dot-leader TOC rows all carry at
/// most one wide internal gap and stay on their existing paths.
pub(super) fn looks_like_table_block_row(line: &str) -> bool {
  if line.split_whitespace().count() < 3 {
    return false;
  }
  count_wide_gaps(line, COLUMN_GAP_SIGNAL) >= 2
}

/// Split a row into cells, treating a run of `CELL_SPLIT_GAP`+ spaces as the
/// boundary between columns and keeping single spaces inside a cell.
fn tokenize_cells(line: &str) -> Vec<Cell> {
  let chars: Vec<char> = line.chars().collect();
  let n = chars.len();
  let mut cells = Vec::new();
  let mut i = 0usize;
  while i < n {
    while i < n && chars[i] == ' ' {
      i += 1;
    }
    if i >= n {
      break;
    }
    let start = i;
    let mut text = String::new();
    while i < n {
      if chars[i] == ' ' {
        let mut k = i;
        while k < n && chars[k] == ' ' {
          k += 1;
        }
        if k - i >= CELL_SPLIT_GAP {
          break;
        }
        for _ in i..k {
          text.push(' ');
        }
        i = k;
      } else {
        text.push(chars[i]);
        i += 1;
      }
    }
    let trimmed = text.trim_end().to_string();
    if !trimmed.is_empty() {
      cells.push(Cell { start, text: trimmed });
    }
  }
  cells
}

fn median(mut values: Vec<usize>) -> usize {
  values.sort_unstable();
  values[values.len() / 2]
}

/// One row of the table once it has been mapped onto the recovered grid.
enum Row {
  /// Cleanly placed into the column grid; `cells[i]` is column `i`'s text.
  Grid(Vec<String>),
  /// Could not be placed (column count or offsets disagreed with the rest);
  /// rendered on its own by compressing its gaps so it still fits.
  Raw(String),
}

/// Recover the dominant column count and each column's anchor position from
/// the rows whose cell count matches the mode. Returns `None` when no column
/// structure is confident enough to trust.
fn detect_anchors(parsed: &[Vec<Cell>]) -> Option<Vec<usize>> {
  // Mode of the per-row cell counts, breaking ties toward the larger count.
  let max_cols = parsed.iter().map(Vec::len).max().unwrap_or(0);
  if max_cols < 2 {
    return None;
  }
  let mut best_count = 0usize;
  let mut best_freq = 0usize;
  for k in 2..=max_cols {
    let freq = parsed.iter().filter(|cells| cells.len() == k).count();
    if freq >= best_freq {
      best_freq = freq;
      best_count = k;
    }
  }
  // Require the modal shape to cover at least half the rows, so a stray run of
  // wide-gap prose can't masquerade as a table.
  if best_count < 2 || best_freq * 2 < parsed.len() {
    return None;
  }

  let mut anchors = Vec::with_capacity(best_count);
  for col in 0..best_count {
    let starts: Vec<usize> = parsed
      .iter()
      .filter(|cells| cells.len() == best_count)
      .map(|cells| cells[col].start)
      .collect();
    anchors.push(median(starts));
  }
  Some(anchors)
}

fn nearest_anchor(anchors: &[usize], start: usize) -> usize {
  anchors
    .iter()
    .enumerate()
    .min_by_key(|(_, a)| start.abs_diff(**a))
    .map(|(idx, _)| idx)
    .unwrap_or(0)
}

/// Map a single row's cells onto the grid defined by `anchors`. Rows with the
/// modal column count map by index; shorter rows snap each cell to its nearest
/// anchor. A collision (two cells claiming one column) means the row's offsets
/// disagree with the grid, so it is returned as `Raw`.
fn place_row(cells: &[Cell], anchors: &[usize], original: &str) -> Row {
  let ncols = anchors.len();
  if cells.len() == ncols {
    return Row::Grid(cells.iter().map(|c| c.text.clone()).collect());
  }
  if cells.len() > ncols {
    return Row::Raw(original.to_string());
  }
  let mut slots: Vec<Option<String>> = vec![None; ncols];
  for cell in cells {
    let col = nearest_anchor(anchors, cell.start);
    if slots[col].is_some() {
      return Row::Raw(original.to_string());
    }
    slots[col] = Some(cell.text.clone());
  }
  Row::Grid(slots.into_iter().map(Option::unwrap_or_default).collect())
}

/// Word-wrap `text` into chunks no wider than `width`, hard-breaking any single
/// word that is itself too long. Always returns at least one (possibly empty)
/// line.
fn wrap_cell(text: &str, width: usize) -> Vec<String> {
  if width == 0 {
    return vec![String::new()];
  }
  if char_len(text) <= width {
    return vec![text.to_string()];
  }
  let mut lines = Vec::new();
  let mut current = String::new();
  for word in text.split(' ') {
    let mut word = word.to_string();
    // Hard-break a single oversized word.
    while char_len(&word) > width {
      let head: String = word.chars().take(width).collect();
      if current.is_empty() {
        lines.push(head.clone());
      } else {
        lines.push(current.clone());
        current.clear();
        lines.push(head.clone());
      }
      word = word.chars().skip(width).collect();
    }
    if current.is_empty() {
      current = word;
    } else if char_len(&current) + 1 + char_len(&word) <= width {
      current.push(' ');
      current.push_str(&word);
    } else {
      lines.push(std::mem::take(&mut current));
      current = word;
    }
  }
  if !current.is_empty() || lines.is_empty() {
    lines.push(current);
  }
  lines
}

/// Choose a per-column width budget whose total (plus single-space gaps) fits
/// `line_width`, shrinking the widest columns first so narrow value columns
/// keep their content and only the roomy label column wraps.
fn allocate_widths(natural: &[usize], line_width: usize) -> Vec<usize> {
  let ncols = natural.len();
  let gaps = ncols.saturating_sub(1) * MIN_GAP;
  let mut widths = natural.to_vec();
  let budget = line_width.saturating_sub(gaps).max(ncols);
  loop {
    let total: usize = widths.iter().sum();
    if total <= budget {
      break;
    }
    // Shrink the current widest column by one; never below a single char.
    let (idx, maxw) =
      widths.iter().enumerate().max_by_key(|(_, w)| **w).unwrap();
    if *maxw <= 1 {
      break;
    }
    widths[idx] -= 1;
  }
  widths
}

/// Render the gridded rows with the given per-column widths and gap, wrapping
/// cells that exceed their column width onto stacked continuation lines.
fn render_grid_row(
  cells: &[String],
  widths: &[usize],
  gap: usize,
) -> Vec<String> {
  let wrapped: Vec<Vec<String>> =
    cells.iter().zip(widths).map(|(text, &w)| wrap_cell(text, w)).collect();
  let height = wrapped.iter().map(Vec::len).max().unwrap_or(1);
  let gap_str = " ".repeat(gap);
  let mut out = Vec::with_capacity(height);
  for line_idx in 0..height {
    let mut parts: Vec<String> = Vec::with_capacity(cells.len());
    for (col, &w) in widths.iter().enumerate() {
      let piece = wrapped[col].get(line_idx).map(String::as_str).unwrap_or("");
      let pad = w.saturating_sub(char_len(piece));
      parts.push(format!("{piece}{}", " ".repeat(pad)));
    }
    out.push(parts.join(&gap_str).trim_end().to_string());
  }
  out
}

/// Lay a row's cells back out on a single line, picking the largest uniform gap
/// (down to one space) that still fits `line_width`. Used for rows that don't
/// fit the grid and as the whole-block fallback when no grid is detected.
fn compress_row(line: &str, line_width: usize) -> String {
  let cells = tokenize_cells(line);
  if cells.len() < 2 {
    return line.to_string();
  }
  let text_total: usize = cells.iter().map(|c| char_len(&c.text)).sum();
  let n_gaps = cells.len() - 1;
  let gap = if text_total + n_gaps * PREFERRED_GAP <= line_width {
    PREFERRED_GAP
  } else {
    MIN_GAP
  };
  let gap_str = " ".repeat(gap);
  cells.iter().map(|c| c.text.as_str()).collect::<Vec<_>>().join(&gap_str)
}

/// Re-render a buffered run of table rows so it fits `line_width`.
///
/// Caller guarantees at least one row is wider than `line_width`; rows that
/// already fit are still re-aligned so the whole block shares one grid.
pub(super) fn render_table_block(
  rows: &[String],
  line_width: usize,
) -> Vec<String> {
  let parsed: Vec<Vec<Cell>> = rows.iter().map(|r| tokenize_cells(r)).collect();

  let Some(anchors) = detect_anchors(&parsed) else {
    // No trustworthy grid: keep every row on one line by compressing gaps.
    return rows.iter().map(|r| compress_row(r, line_width)).collect();
  };

  let placed: Vec<Row> = parsed
    .iter()
    .zip(rows)
    .map(|(cells, original)| place_row(cells, &anchors, original))
    .collect();

  // Natural width per column across the gridded rows.
  let ncols = anchors.len();
  let mut natural = vec![0usize; ncols];
  for row in &placed {
    if let Row::Grid(cells) = row {
      for (col, text) in cells.iter().enumerate() {
        natural[col] = natural[col].max(char_len(text));
      }
    }
  }

  // Pick a gap: prefer two spaces, fall back to one, otherwise wrap cells.
  let sum_natural: usize = natural.iter().sum();
  let (widths, gap) = if sum_natural + PREFERRED_GAP * (ncols - 1) <= line_width
  {
    (natural.clone(), PREFERRED_GAP)
  } else if sum_natural + MIN_GAP * (ncols - 1) <= line_width {
    (natural.clone(), MIN_GAP)
  } else {
    (allocate_widths(&natural, line_width), MIN_GAP)
  };

  let mut out = Vec::with_capacity(rows.len());
  for row in &placed {
    match row {
      Row::Grid(cells) => out.extend(render_grid_row(cells, &widths, gap)),
      Row::Raw(original) => out.push(compress_row(original, line_width)),
    }
  }
  out
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn detects_multi_column_grid_rows() {
    assert!(looks_like_table_block_row(
      "TWI/I2C       1 (1)        2 (1)        2"
    ));
    // Two-column option table: only one wide gap, must stay off the table path.
    assert!(!looks_like_table_block_row(
      " -p          Show the patch introduced with each commit."
    ));
    // Ordinary prose.
    assert!(!looks_like_table_block_row(
      "This is just a normal sentence with words."
    ));
  }

  #[test]
  fn aligns_overflowing_datasheet_table_into_a_grid() {
    // Real layout-extractor output for the AVR128DB "...continued" device
    // comparison table from issue #92.
    let block = vec![
      "AVR32DB28                AVR32DB32                AVR32DB48                AVR64DB64".to_string(),
      "Feature                                                  AVR64DB28                AVR64DB32                AVR64DB48                AVR64DB64".to_string(),
      "                                                            AVR128DB28              AVR128DB32              AVR128DB48".to_string(),
      "12-bit differential ADC (channels)                  1 (9)                          1 (13)                        1                              1 (2)".to_string(),
      "10-bit DAC (outputs)                                  1 (1)                          1 (1)                          1 (1)                          1 (1)".to_string(),
      "Peripheral Touch Controller (PTC)                 -                               -                               -                               -".to_string(),
      "Configurable Custom Logic Look-up               4                              4                              6                              6".to_string(),
      "Event System channels (sync)                      8                              8                              10                             10".to_string(),
      "General Purpose I/O                                  55/54 (2)                    22/21 (2)                    26/25 (2)                    26/25 (2)".to_string(),
    ];
    let out = render_table_block(&block, 80);

    // Every emitted line fits the width — nothing is dumped onto a stray
    // short-indented continuation line.
    for line in &out {
      assert!(
        char_len(line) <= 80,
        "line exceeds width: {line:?} ({} chars)",
        char_len(line)
      );
    }

    // Each data row keeps all four of its values on one line.
    let adc = out
      .iter()
      .find(|l| l.contains("12-bit differential ADC"))
      .expect("ADC row present");
    assert!(
      adc.contains("1 (9)") && adc.contains("1 (13)") && adc.contains("1 (2)"),
      "ADC row should keep its values on one line, got: {adc:?}"
    );

    // Columns are aligned: the first value column starts at the same offset
    // for every full data row.
    let value_col = |needle: &str| -> usize {
      let line = out.iter().find(|l| l.contains(needle)).unwrap();
      // offset of the first value cell = end of the label + gap run.
      line.find(needle).unwrap()
    };
    let dac_pos = value_col("10-bit DAC (outputs)");
    let ptc_pos = value_col("Peripheral Touch Controller");
    let dac_line = out.iter().find(|l| l.contains("10-bit DAC")).unwrap();
    let ptc_line = out.iter().find(|l| l.contains("Peripheral Touch")).unwrap();
    let dac_val = dac_line[dac_pos..].find("1 (1)").unwrap();
    let ptc_val = ptc_line[ptc_pos..].find('-').unwrap();
    assert_eq!(
      dac_pos + dac_val,
      ptc_pos + ptc_val,
      "value columns should be vertically aligned, got:\n{dac_line}\n{ptc_line}"
    );
  }

  #[test]
  fn falls_back_to_single_line_compression_without_a_grid() {
    // No column count covers half the rows, so there is no trustworthy grid;
    // each row must still collapse onto a single fitting line rather than
    // wrap-dumping its overflow onto a stray continuation line.
    let block = vec![
      "aa          bb".to_string(),
      "cc          dd          ee".to_string(),
      "ff          gg          hh          ii".to_string(),
      "jj          kk          ll          mm          nn          oo          pp".to_string(),
    ];
    let out = render_table_block(&block, 40);
    assert_eq!(out.len(), 4, "one output line per input row, got: {out:?}");
    // The over-wide last row is compressed onto one single line.
    assert!(
      out[3].starts_with("jj") && out[3].contains("pp"),
      "wide row should stay on one line, got: {:?}",
      out[3]
    );
  }

  #[test]
  fn wraps_cells_when_even_minimal_gaps_overflow() {
    // A label column too wide to fit alongside the value columns must wrap
    // inside its column instead of pushing the row past the width.
    let block = vec![
      "Short            1        2        3".to_string(),
      "A very long descriptive label that will not fit            10       20       30".to_string(),
    ];
    let out = render_table_block(&block, 40);
    for line in &out {
      assert!(char_len(line) <= 40, "line too wide: {line:?}");
    }
    assert!(
      out.len() > block.len(),
      "long label should wrap onto extra lines, got: {out:?}"
    );
  }
}
