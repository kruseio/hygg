use crate::stream::types::PDF_TEXT_PT_PER_CHAR;

/// Build a text blob from pdf_oxide's positional `TextLine` output.
///
/// Lines are returned in a roughly visual order but adjacent rows can
/// collide when text is laid out in cells (table rows) or columns. We
/// sort by y descending (PDF origin is bottom-left, so top of page is the
/// largest y), then walk the list collecting lines that share a row into
/// a single output line, sorted left-to-right within the row.
pub(crate) fn extract_page_text_lines(
  doc: &pdf_oxide::PdfDocument,
  page_0based: usize,
) -> Option<String> {
  let mut lines = doc.extract_text_lines(page_0based).ok()?;
  if lines.is_empty() {
    return None;
  }

  // Sort top-to-bottom, then left-to-right.
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

  // Threshold below which two lines are considered to be on the same row.
  // pdf_oxide's line bboxes for the same baseline tend to differ by < 1pt
  // even with mixed font sizes; 3pt comfortably absorbs that noise without
  // merging adjacent rows (which are typically separated by 10+pt).
  const SAME_ROW_TOL: f32 = 3.0;

  // ~5 pt per char is a rough monospace approximation that lands within
  // a column or two of correct on body fonts in the PDFs we test against.
  // Cap the resulting indent so an outlier x-coordinate can't produce a
  // multi-line waste of whitespace.
  const MAX_INDENT_CHARS: usize = 20;

  // Build rows first as `(anchor_y, row_left, body_text)` so we can
  // post-process before producing the final string (drop isolated
  // page-number rows, drop running headers, recompute page_left after
  // dropping outliers, insert paragraph-break blank lines, etc.).
  // `body_text` is the row content WITHOUT its leading indent — the indent
  // is applied later from `(row_left - page_left)` once page_left has been
  // settled.
  let mut rows: Vec<(f32, f32, String)> = Vec::new();
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
      // Walk every word across every TextLine in this row left-to-right
      // and insert spacing proportional to the bbox gap between adjacent
      // words. `TextLine::text` joins words with a single space and so
      // collapses the wide column gaps that TOC pages depend on — without
      // those gaps `parse_aligned_toc_row_start` can't split a row like
      // `1.1     About This Book     25` into prefix/title/page-number.
      let mut body = String::with_capacity(64);
      let mut prev_right: Option<f32> = None;
      for line in row.iter() {
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
      rows.push((row_anchor_y, row_left, body));
      row_start = i;
      if i < lines.len() {
        row_anchor_y = lines[i].bbox.top();
      }
    }
  }

  // Drop the top/bottom row if it's an isolated digits-only run — almost
  // certainly the page-number header/footer. The old `>=20 leading ws`
  // sanitize rule didn't survive positional extraction (we recompute
  // indents ourselves), so this is the only thing standing between the
  // page-number "5" / "6" / "7" rows and the reader.
  //
  // We deliberately do NOT drop short alphabetic running headers
  // (`Contents`, `Figures`, etc.) here — on some pages those same words
  // are the actual centered chapter title, and we can't tell them apart
  // by isolation alone. The sanitize pass dedups them by exact text after
  // the first occurrence is registered as a centered heading.
  const ISOLATED_GAP: f32 = 30.0;
  // Loop so a page that has BOTH a "6" page-number AND a centered title
  // above the body still strips the page number; the title stays.
  while rows.len() >= 2
    && is_digits_only(&rows[0].2)
    && (rows[0].0 - rows[1].0).abs() > ISOLATED_GAP
  {
    rows.remove(0);
  }
  while rows.len() >= 2 {
    let last = rows.len() - 1;
    if is_digits_only(&rows[last].2)
      && (rows[last - 1].0 - rows[last].0).abs() > ISOLATED_GAP
    {
      rows.remove(last);
    } else {
      break;
    }
  }

  // Page body left margin. We need a value that's stable across pages so
  // facing-page TOCs (where the running header lives in a different x
  // column than body content) don't produce different indents for the
  // same logical content. Strategy: take the leftmost x that's "popular"
  // — bucket every row's left edge at 1pt resolution and use the smallest
  // bucket that has more than one row. Singleton positions (centered
  // titles, lone running headers, isolated captions) get filtered out and
  // can no longer pull the margin to the left.
  let mut buckets: std::collections::HashMap<i32, usize> =
    std::collections::HashMap::new();
  for (_, row_left, _) in &rows {
    let key = row_left.round() as i32;
    *buckets.entry(key).or_insert(0) += 1;
  }
  let popular_min = buckets
    .iter()
    .filter(|(_, count)| **count >= 2)
    .map(|(k, _)| *k as f32)
    .fold(f32::INFINITY, f32::min);
  let page_left = if popular_min.is_finite() {
    popular_min
  } else {
    rows.iter().map(|(_, x, _)| *x).fold(f32::INFINITY, f32::min)
  };

  // Paragraph / code-block boundaries: pdf_oxide gives us no signal for
  // these — adjacent rows just have their y values, and runs of body text
  // sit ~13-16pt apart while a paragraph break or heading-to-prose
  // transition leaves a 25-35pt gap. Emit a blank line at every gap
  // that's noticeably larger than the page's *typical* line gap, so
  // downstream re-justification (and the reader visually) gets the same
  // paragraph shape pdf-extract used to produce.
  //
  // "Typical" here is the *mode* of the gap distribution bucketed at 2pt,
  // not the median or mean — within-block line spacing is by far the most
  // common gap (most rows are body text on most pages), so the mode tracks
  // it directly. Mean and median both pick up an upward bias from the few
  // legitimate paragraph breaks they're trying to detect.
  let gaps: Vec<f32> =
    rows.windows(2).map(|w| (w[0].0 - w[1].0).max(0.0)).collect();
  let para_threshold = paragraph_gap_threshold(&gaps);

  let mut output =
    String::with_capacity(rows.iter().map(|(_, _, s)| s.len() + 8).sum());
  for i in 0..rows.len() {
    if i > 0 && gaps[i - 1] > para_threshold {
      output.push('\n');
    }
    let (_, row_left, body) = &rows[i];
    let indent_chars = (((row_left - page_left) / PDF_TEXT_PT_PER_CHAR).round())
      .max(0.0) as usize;
    let indent_chars = indent_chars.min(MAX_INDENT_CHARS);
    for _ in 0..indent_chars {
      output.push(' ');
    }
    output.push_str(body);
    output.push('\n');
  }
  Some(output)
}

pub(crate) fn push_pdf_word_gap(
  body: &mut String,
  prev_right: Option<f32>,
  word_left: f32,
  pt_per_char: f32,
) {
  let Some(prev_right) = prev_right else {
    return;
  };

  let gap_pt = word_left - prev_right;
  if gap_pt <= pt_per_char * 0.25 {
    return;
  }

  // Capped for the same reason layout_text_output::write_n_spaces caps at 200:
  // the gap is a subtraction of two document-supplied coordinates, not a
  // measurement of anything real. A 612pt page is ~122 cells at 5pt/char, so no
  // honest intra-row gap comes near this; a hostile one is unbounded, and
  // `f32 as usize` saturates rather than wrapping — an infinite gap_pt asks for
  // usize::MAX spaces in a String. Downstream justify re-wraps at `col`
  // regardless, so the cap is invisible to real documents.
  const MAX_GAP_CHARS: usize = 200;
  let gap_chars =
    ((gap_pt / pt_per_char).round() as usize).clamp(1, MAX_GAP_CHARS);
  body.extend(std::iter::repeat_n(' ', gap_chars));
}

pub(crate) fn is_digits_only(s: &str) -> bool {
  let t = s.trim();
  !t.is_empty() && t.chars().all(|c| c.is_ascii_digit())
}

/// Returns "anything bigger than this is a paragraph / block break"
/// derived from the distribution of gaps on the page.
///
/// Strategy: bucket gaps at 2pt resolution, take the most-popular bucket
/// as the within-block line spacing, then return 1.7× that. We ignore
/// gaps under 5pt (intra-row noise from the row-grouping tolerance) when
/// computing the mode. Clamped to [20, 50] pt so a degenerate page (one
/// row, all-equal gaps, etc.) still produces a sane threshold.
fn paragraph_gap_threshold(gaps: &[f32]) -> f32 {
  let mut buckets: std::collections::HashMap<i32, usize> =
    std::collections::HashMap::new();
  for &g in gaps {
    if g >= 5.0 {
      let key = (g / 2.0).round() as i32;
      *buckets.entry(key).or_insert(0) += 1;
    }
  }
  let mode_gap = buckets
    .iter()
    .max_by_key(|(_, c)| *c)
    .map(|(k, _)| (*k as f32) * 2.0)
    .unwrap_or(14.0);
  (mode_gap * 1.7).clamp(20.0, 50.0)
}
