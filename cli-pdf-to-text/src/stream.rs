use std::path::PathBuf;
use std::sync::Arc;

use cli_image_to_ascii::{RenderConfig, render_half_block};
use hygg_shared::normalize_file_path;

use crate::sanitize::sanitize_layout_text;

/// On-demand page extractor backed by a single parsed `pdf_oxide::PdfDocument`.
///
/// pdf_oxide parses the file lazily — `open` does the xref + catalog and
/// returns in tens of milliseconds even on the 31 MB / 1310-page PDF
/// reference, where `lopdf::Document::load` (the old backend) took ~40 s
/// because it eagerly decompressed every content stream. Per-page
/// extraction is sub-millisecond warm, hundreds of micros cold.
///
/// `pdf_oxide::PdfDocument` is `Send + Sync` (its interior-mutable caches
/// are `Mutex`-guarded), so a `PdfStream` can be wrapped in `Arc` and
/// shared between the main thread (rendering the first visible page) and
/// the background loader thread (extracting the rest of the document) the
/// same way the lopdf-backed version was.
pub struct PdfStream {
  canonical_path: PathBuf,
  doc: pdf_oxide::PdfDocument,
  total_pages: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PdfLineKind {
  Text,
  AnsiArt,
}

#[derive(Clone, Debug)]
pub struct PdfRenderedPage {
  pub raw_text: String,
  pub lines: Vec<String>,
  pub line_kinds: Vec<PdfLineKind>,
  pub contains_images: bool,
}

impl PdfStream {
  /// Open a PDF and parse its catalog. Does not extract any page text.
  pub fn open(pdf_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
    let canonical_path = normalize_file_path(pdf_path)?;
    let doc = pdf_oxide::PdfDocument::open(&canonical_path)
      .map_err(|e| format!("pdf_oxide open failed: {e:?}"))?;
    let total_pages = doc
      .page_count()
      .map_err(|e| format!("pdf_oxide page_count failed: {e:?}"))?;
    Ok(Self { canonical_path, doc, total_pages })
  }

  pub fn total_pages(&self) -> usize {
    self.total_pages
  }

  pub fn canonical_path(&self) -> &std::path::Path {
    &self.canonical_path
  }

  /// Extract sanitized text for a single page.
  ///
  /// `page_index` is 1-based to match the historical lopdf-backed API
  /// (the rest of hygg counts pages from 1 in saved progress, status
  /// line, etc.). Returns `None` if the index is out of range, the page
  /// has no extractable text, or extraction panicked. pdf_oxide claims a
  /// 100 % pass rate on its 3 830-PDF corpus, but we still wrap in
  /// `catch_unwind` so a misbehaving page can't take down the background
  /// loader thread and leave every later page stuck on "loading".
  ///
  /// Uses pdf_oxide's positional `extract_text_lines` rather than the
  /// simpler `extract_text`. The former returns each visual line with
  /// its bounding box; we group lines that share a row (overlapping y
  /// ranges) and join them left-to-right. Without that step pdf_oxide
  /// can interleave adjacent TOC entries — "1.3 Foo1.4 Bar 3231" — and
  /// the downstream sanitizer can't recover them.
  pub fn extract_page(&self, page_index: usize) -> Option<String> {
    if page_index == 0 || page_index > self.total_pages {
      return None;
    }
    let doc = &self.doc;
    let page_0based = page_index - 1;
    let raw = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
      extract_page_text_lines(doc, page_0based)
    }))
    .ok()
    .flatten()?;
    if raw.trim().is_empty() {
      return None;
    }
    Some(sanitize_layout_text(&raw))
  }

  pub fn extract_page_with_images(
    &self,
    page_index: usize,
    col: usize,
  ) -> Option<PdfRenderedPage> {
    if page_index == 0 || page_index > self.total_pages {
      return None;
    }

    let raw_text = self.extract_page(page_index).unwrap_or_default();
    let page_0based = page_index - 1;
    let images = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
      self.doc.extract_images(page_0based)
    }))
    .ok()
    .and_then(Result::ok)
    .unwrap_or_default();

    let image_rows =
      render_pdf_images(&self.doc, page_0based, col, images.as_slice());
    if image_rows.is_empty() {
      let PdfPageForAnsi { lines, line_kinds } =
        text_only_page_lines(&raw_text, col);
      return Some(PdfRenderedPage {
        raw_text,
        lines,
        line_kinds,
        contains_images: false,
      });
    }

    let text_rows = sanitized_visual_text_rows(&raw_text, col);

    let PdfPageForAnsi { lines, line_kinds } =
      compose_visual_page(text_rows, image_rows);
    Some(PdfRenderedPage { raw_text, lines, line_kinds, contains_images: true })
  }
}

struct PdfPageForAnsi {
  lines: Vec<String>,
  line_kinds: Vec<PdfLineKind>,
}

#[derive(Clone)]
struct VisualTextRow {
  top: f32,
  left: f32,
  text: String,
}

struct VisualImageRows {
  top: f32,
  left_cells: usize,
  lines: Vec<String>,
}

fn text_only_page_lines(raw_text: &str, col: usize) -> PdfPageForAnsi {
  let lines = cli_justify::justify_pdf_page(raw_text, col).lines;
  let line_kinds = vec![PdfLineKind::Text; lines.len()];
  PdfPageForAnsi { lines, line_kinds }
}

fn render_pdf_images(
  doc: &pdf_oxide::PdfDocument,
  page_0based: usize,
  col: usize,
  images: &[pdf_oxide::extractors::PdfImage],
) -> Vec<VisualImageRows> {
  if col == 0 {
    return Vec::new();
  }
  let (page_left, page_width) = doc
    .get_page_media_box(page_0based)
    .ok()
    .map(|(llx, _, urx, _)| (llx, (urx - llx).abs()))
    .filter(|(_, w)| *w > 0.0)
    .unwrap_or((0.0, 612.0));

  let mut out = Vec::new();
  for image in images {
    let Some(bbox) = image.bbox() else {
      continue;
    };
    if bbox.width <= 0.0 || bbox.height <= 0.0 {
      continue;
    }
    let Ok(dynamic_image) = image.to_dynamic_image() else {
      continue;
    };
    let left_cells = pdf_x_to_cells(bbox.left(), page_left, page_width, col);
    let left_cells = left_cells.min(col.saturating_sub(1));
    let width_cells = pdf_width_to_cells(bbox.width, page_width, col);
    let width_cells = width_cells.max(1).min(col.saturating_sub(left_cells));
    if width_cells == 0 {
      continue;
    }
    let height_rows =
      pdf_image_height_rows(bbox.width, bbox.height, width_cells);
    let lines = render_half_block(
      &dynamic_image,
      RenderConfig::new(Some(width_cells as u32), Some(height_rows as u32)),
    );
    if lines.is_empty() {
      continue;
    }
    out.push(VisualImageRows { top: bbox.top(), left_cells, lines });
  }
  out
}

fn pdf_x_to_cells(
  x: f32,
  page_left: f32,
  page_width: f32,
  col: usize,
) -> usize {
  if page_width <= 0.0 || col == 0 {
    return 0;
  }
  (((x - page_left).max(0.0) / page_width) * col as f32).round() as usize
}

fn pdf_width_to_cells(width: f32, page_width: f32, col: usize) -> usize {
  if page_width <= 0.0 || col == 0 {
    return 0;
  }
  ((width.max(0.0) / page_width) * col as f32).round() as usize
}

fn pdf_image_height_rows(
  bbox_width: f32,
  bbox_height: f32,
  width_cells: usize,
) -> usize {
  if bbox_width <= 0.0 || bbox_height <= 0.0 || width_cells == 0 {
    return 1;
  }
  ((bbox_height / bbox_width) * width_cells as f32).round().max(1.0) as usize
}

fn compose_visual_page(
  text_rows: Vec<VisualTextRow>,
  image_rows: Vec<VisualImageRows>,
) -> PdfPageForAnsi {
  enum Event {
    Text(VisualTextRow),
    Image(VisualImageRows),
  }

  let mut events: Vec<Event> =
    Vec::with_capacity(text_rows.len() + image_rows.len());
  events.extend(text_rows.into_iter().map(Event::Text));
  events.extend(image_rows.into_iter().map(Event::Image));
  events.sort_by(|a, b| {
    let a_top = match a {
      Event::Text(row) => row.top,
      Event::Image(row) => row.top,
    };
    let b_top = match b {
      Event::Text(row) => row.top,
      Event::Image(row) => row.top,
    };
    b_top.partial_cmp(&a_top).unwrap_or(std::cmp::Ordering::Equal)
  });

  let page_left = events
    .iter()
    .filter_map(|event| match event {
      Event::Text(row) if !row.text.trim().is_empty() => Some(row.left),
      _ => None,
    })
    .fold(f32::INFINITY, f32::min);
  let page_left = if page_left.is_finite() { page_left } else { 0.0 };

  let mut lines = Vec::new();
  let mut line_kinds = Vec::new();
  for event in events {
    match event {
      Event::Text(row) => {
        if row.text.trim().is_empty() {
          continue;
        }
        let indent =
          (((row.left - page_left) / 5.0).round()).max(0.0).min(20.0) as usize;
        lines.push(format!("{}{}", " ".repeat(indent), row.text));
        line_kinds.push(PdfLineKind::Text);
      }
      Event::Image(row) => {
        let indent = " ".repeat(row.left_cells);
        for line in row.lines {
          lines.push(format!("{indent}{line}\x1b[0m"));
          line_kinds.push(PdfLineKind::AnsiArt);
        }
      }
    }
  }

  if lines.is_empty() {
    lines.push(String::new());
    line_kinds.push(PdfLineKind::Text);
  }

  PdfPageForAnsi { lines, line_kinds }
}

fn sanitized_visual_text_rows(
  raw_text: &str,
  col: usize,
) -> Vec<VisualTextRow> {
  cli_justify::justify_pdf_page(raw_text, col)
    .lines
    .into_iter()
    .enumerate()
    .map(|(idx, text)| VisualTextRow { top: -(idx as f32), left: 0.0, text })
    .collect()
}

/// Build a text blob from pdf_oxide's positional `TextLine` output.
///
/// Lines are returned in a roughly visual order but adjacent rows can
/// collide when text is laid out in cells (table rows) or columns. We
/// sort by y descending (PDF origin is bottom-left, so top of page is the
/// largest y), then walk the list collecting lines that share a row into
/// a single output line, sorted left-to-right within the row.
fn extract_page_text_lines(
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
  const PT_PER_CHAR: f32 = 5.0;
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
          if let Some(pr) = prev_right {
            let gap_pt = (word.bbox.left() - pr).max(0.0);
            let gap_chars = ((gap_pt / PT_PER_CHAR).round() as usize).max(1);
            for _ in 0..gap_chars {
              body.push(' ');
            }
          }
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
    let indent_chars =
      (((row_left - page_left) / PT_PER_CHAR).round()).max(0.0) as usize;
    let indent_chars = indent_chars.min(MAX_INDENT_CHARS);
    for _ in 0..indent_chars {
      output.push(' ');
    }
    output.push_str(body);
    output.push('\n');
  }
  Some(output)
}

fn is_digits_only(s: &str) -> bool {
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

/// Convenience wrapper so callers can hold a cheap shared handle.
pub type SharedPdfStream = Arc<PdfStream>;

#[cfg(test)]
mod tests {
  use super::*;
  use std::path::Path;

  #[test]
  fn opens_and_extracts_individual_pages() {
    let pdf_path = Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("../test-data/pdf/progit-1-50.pdf");
    if !pdf_path.exists() {
      return;
    }
    let stream = PdfStream::open(pdf_path.to_str().expect("utf-8 path"))
      .expect("PdfStream should open valid test PDF");
    assert!(stream.total_pages() > 0, "test PDF should report pages");

    // Scan a few early pages — at least one should produce real text.
    // (The first page of progit is a title/cover with minimal text.)
    let scan_upto = stream.total_pages().min(5);
    let mut any_non_empty = false;
    for p in 1..=scan_upto {
      if let Some(text) = stream.extract_page(p)
        && !text.trim().is_empty()
      {
        any_non_empty = true;
        break;
      }
    }
    assert!(
      any_non_empty,
      "at least one of the first {scan_upto} pages should extract non-empty text"
    );
  }

  /// Regression: progit page 43 (the "Skipping the Staging Area" page)
  /// used to lose all paragraph breaks because pdf_oxide's text-line API
  /// doesn't signal them — and the standalone "37" page-number footer
  /// used to leak into content because the existing sanitize.rs heuristic
  /// for footer numbers requires ≥20 chars of leading whitespace, which
  /// our positional row builder strips. Verify both stay fixed.
  #[test]
  fn progit_paragraph_breaks_and_page_footer() {
    let pdf_path =
      Path::new(env!("CARGO_MANIFEST_DIR")).join("../test-data/pdf/progit.pdf");
    if !pdf_path.exists() {
      return;
    }
    let stream = PdfStream::open(pdf_path.to_str().expect("utf-8 path"))
      .expect("PdfStream should open progit");
    let text =
      stream.extract_page(43).expect("progit page 43 should produce text");

    // Page-number footer must not leak through.
    let lines: Vec<&str> = text.lines().collect();
    assert!(
      !lines.iter().any(|l| l.trim() == "37"),
      "isolated page-number footer '37' should be stripped, got:\n{text}"
    );

    // The "Alternatively, you can type your commit message" sentence
    // starts a new paragraph after "and diff stripped out)." — there
    // should be a blank line between them so the reflowed output keeps
    // paragraph structure.
    let alt_pos = text
      .find("Alternatively, you can type your commit message")
      .expect("expected sentence on page 43");
    let before = &text[..alt_pos];
    assert!(
      before.trim_end().ends_with("and diff stripped out)."),
      "text immediately before 'Alternatively…' should end the previous \
       paragraph, got:\n…{}…",
      &before[before.len().saturating_sub(80)..]
    );
    let trailing_newlines =
      before.as_bytes().iter().rev().take_while(|&&b| b == b'\n').count();
    assert!(
      trailing_newlines >= 2,
      "expected at least one blank line before 'Alternatively…' \
       (a paragraph break), got {trailing_newlines} trailing newlines"
    );
  }

  /// Regression: the pdf reference 1.7 TOC interleaves two adjacent
  /// section headers because `extract_text` collapses lines without
  /// regard to their bounding boxes. `extract_text_lines` + the
  /// row-grouping in `extract_page_text_lines` is what fixes it, so make
  /// sure section labels stay on their own lines for a TOC-shaped page.
  #[test]
  fn toc_section_labels_stay_separate() {
    let pdf_path = Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("../test-data/pdf/pdfreference1.7old.pdf");
    if !pdf_path.exists() {
      return;
    }
    let stream = PdfStream::open(pdf_path.to_str().expect("utf-8 path"))
      .expect("PdfStream should open the reference PDF");
    // Page 5 (1-based) is the contents page.
    let text = stream.extract_page(5).expect("page 5 should produce text");
    let lines: Vec<&str> = text.lines().collect();
    // Word-bbox-derived spacing now preserves the wide TOC gap between the
    // section title and its trailing page number, so the trimmed row keeps
    // multiple spaces between them. Match either spacing shape.
    let normalize_spaces =
      |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
      lines
        .iter()
        .any(|l| normalize_spaces(l.trim()) == "1.3 Related Publications 31"),
      "section 1.3 should be on its own line, got:\n{text}"
    );
    assert!(
      lines
        .iter()
        .any(|l| normalize_spaces(l.trim()) == "1.4 Intellectual Property 32"),
      "section 1.4 should be on its own line, got:\n{text}"
    );
    // The collapsing bug previously produced this run-on string.
    assert!(
      !text.contains("1.3 Related Publications1.4"),
      "section labels must not be concatenated, got:\n{text}"
    );
  }

  #[test]
  fn visual_composition_orders_text_and_ansi_art_with_metadata() {
    let text_rows = vec![
      VisualTextRow { top: 90.0, left: 50.0, text: "after image".to_string() },
      VisualTextRow {
        top: 200.0,
        left: 50.0,
        text: "before image".to_string(),
      },
    ];
    let image_rows = vec![VisualImageRows {
      top: 150.0,
      left_cells: 4,
      lines: vec!["\x1b[38;2;1;2;3m\x1b[48;2;4;5;6m▀\x1b[0m".into()],
    }];

    let page = compose_visual_page(text_rows, image_rows);

    assert_eq!(
      page.line_kinds,
      vec![PdfLineKind::Text, PdfLineKind::AnsiArt, PdfLineKind::Text,]
    );
    assert_eq!(page.lines[0], "before image");
    assert!(page.lines[1].starts_with("    \x1b[38;2;1;2;3m"));
    assert!(page.lines[1].ends_with("\x1b[0m"));
    assert_eq!(page.lines[2], "after image");
  }

  #[test]
  fn text_only_ansi_page_keeps_every_line_text_marked() {
    let page = text_only_page_lines("one two three", 10);

    assert!(!page.lines.is_empty());
    assert_eq!(page.line_kinds, vec![PdfLineKind::Text; page.lines.len()]);
  }

  #[test]
  fn pdf_cell_mapping_accounts_for_media_box_origin() {
    assert_eq!(pdf_x_to_cells(100.0, 100.0, 500.0, 80), 0);
    assert_eq!(pdf_x_to_cells(350.0, 100.0, 500.0, 80), 40);
    assert_eq!(pdf_width_to_cells(125.0, 500.0, 80), 20);
  }

  #[test]
  fn pdf_image_height_uses_display_bbox_aspect_ratio() {
    assert_eq!(pdf_image_height_rows(100.0, 50.0, 20), 10);
    assert_eq!(pdf_image_height_rows(100.0, 200.0, 20), 40);
    assert_eq!(pdf_image_height_rows(0.0, 200.0, 20), 1);
  }
}
