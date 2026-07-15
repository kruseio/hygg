//! Batch page assembly for the flattened-lines PDF path (PWA/GUI import).
//!
//! The terminal reader streams pages and stitches them in
//! `cli-text-reader`'s `PdfStreamingState::flat_lines`: adjacent pages whose
//! paragraph spans the break are dehyphenated and re-justified into one seam,
//! and the inter-page gap is 0 or 1 blank rather than always 1. A frontend that
//! extracts every page up front (`pdf_bytes_to_lines_paged`) must produce the
//! *same* flat buffer, or a page-local resume anchor synced from the CLI lands
//! a few lines off wherever a paragraph crosses a page boundary.
//!
//! This module reproduces that assembly for the all-pages-loaded case. It
//! mirrors `LoadedPage::from_rendered` (per-page rendering) and `flat_lines`
//! (seam stitching + separators) line for line; a golden test in
//! `cli-text-reader` asserts the two stay byte-identical.

use cli_justify::{
  PartialParagraph, PdfPageJustified, inter_page_blank_count, justify_pdf_page,
  justify_pdf_seam,
};

use crate::stream::{PdfLineKind, PdfStream};

/// One page's rendered data — the subset of `LoadedPage` the flat assembly
/// needs.
struct PageData {
  standalone: Vec<String>,
  kinds: Vec<PdfLineKind>,
  head: Option<PartialParagraph>,
  tail: Option<PartialParagraph>,
  contains_images: bool,
}

impl PageData {
  /// Mirror `LoadedPage::from_rendered`: an image page keeps its interleaved
  /// ASCII-art rows and disables seam partials; a text page re-justifies its
  /// raw text with `justify_pdf_page` (exactly as `from_raw` does), exposing
  /// the head/tail partials that drive cross-page seam stitching.
  fn render(stream: &PdfStream, page_1based: usize, col: usize) -> Self {
    let Some(rendered) = stream.extract_page_with_images(page_1based, col)
    else {
      return Self {
        standalone: Vec::new(),
        kinds: Vec::new(),
        head: None,
        tail: None,
        contains_images: false,
      };
    };

    if !rendered.contains_images {
      let PdfPageJustified { lines, head_partial, tail_partial } =
        justify_pdf_page(&rendered.raw_text, col);
      let kinds = vec![PdfLineKind::Text; lines.len()];
      return Self {
        standalone: lines,
        kinds,
        head: head_partial,
        tail: tail_partial,
        contains_images: false,
      };
    }

    let mut kinds = rendered.line_kinds;
    if kinds.len() != rendered.lines.len() {
      kinds = vec![PdfLineKind::Text; rendered.lines.len()];
    }
    Self {
      standalone: rendered.lines,
      kinds,
      head: None,
      tail: None,
      contains_images: true,
    }
  }
}

/// Blank separator lines before a page (mirrors `separator_before_page` for two
/// loaded pages): a single blank when either page carries images, else the
/// content-aware 0-or-1 the reader uses to keep sibling lists/captions tight.
fn separator_before(prev: &PageData, this: &PageData) -> usize {
  if prev.contains_images || this.contains_images {
    return 1;
  }
  inter_page_blank_count(&prev.standalone, &this.standalone)
}

/// Assemble every page into the flat `(line, kind)` buffer plus each 1-based
/// page's first-line index, applying the same seam stitching and inter-page
/// spacing the streaming reader's `flat_lines` produces once all pages are
/// loaded — so the PWA/GUI flat buffer is byte-identical to the CLI's and a
/// page-local resume anchor resolves to the same content in every client.
pub(crate) fn assemble_paged(
  stream: &PdfStream,
  col: usize,
) -> (Vec<(String, PdfLineKind)>, Vec<usize>) {
  let total = stream.total_pages();
  let pages: Vec<PageData> =
    (1..=total).map(|p| PageData::render(stream, p, col)).collect();

  let mut out: Vec<(String, PdfLineKind)> = Vec::new();
  let mut page_starts = Vec::with_capacity(total);
  // The previous page emitted a seam that stitches into this one, so no
  // separator is inserted before it — the seam *is* the connection.
  let mut last_was_seam = false;

  for idx in 0..total {
    if idx > 0 && !last_was_seam {
      for _ in 0..separator_before(&pages[idx - 1], &pages[idx]) {
        out.push((String::new(), PdfLineKind::Text));
      }
    }
    last_was_seam = false;
    // Page start is recorded after any leading separator, matching
    // `line_start_for_page` (which attributes the gap to the preceding page).
    page_starts.push(out.len());

    let page = &pages[idx];
    let prev_tail = idx.checked_sub(1).and_then(|i| pages[i].tail.as_ref());
    let next_head = pages.get(idx + 1).and_then(|n| n.head.as_ref());

    // A head partial is dropped only when the previous page actually had a
    // tail to stitch it onto; otherwise it stays as this page's own content.
    let head_skip = match (&page.head, prev_tail) {
      (Some(head), Some(_)) => head.line_count,
      _ => 0,
    };
    let (tail_skip, seam) = match (&page.tail, next_head) {
      (Some(tail), Some(next_head)) => (
        tail.line_count,
        Some(justify_pdf_seam(&tail.raw_text, &next_head.raw_text, col)),
      ),
      _ => (0, None),
    };

    if !page.standalone.is_empty() {
      let end = page.standalone.len().saturating_sub(tail_skip);
      let start = head_skip.min(end);
      for i in start..end {
        out.push((page.standalone[i].clone(), page.kinds[i]));
      }
    }
    if let Some(seam) = seam {
      for line in seam {
        out.push((line, PdfLineKind::Text));
      }
      last_was_seam = true;
    }
  }

  if out.is_empty() {
    out.push((String::new(), PdfLineKind::Text));
  }
  (out, page_starts)
}
