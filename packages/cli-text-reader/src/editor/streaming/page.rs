use cli_justify::{
  PdfPageJustified, inter_page_blank_count, justify_pdf_page, justify_pdf_seam,
};
use cli_pdf_to_text::{PdfLineKind, PdfRenderedPage};

use super::types::LoadedPage;

impl LoadedPage {
  /// Number of lines this page will contribute to the flat line buffer
  /// taking neighbour-driven stitching into account.
  ///
  /// `next_loading` is `true` when a slot follows this page in the page
  /// table but isn't loaded yet, so the per-page count agrees with what
  /// `flat_lines` will emit for the not-yet-known next page (a single
  /// blank separator placeholder).
  pub fn rendered_line_count(
    &self,
    prev: Option<&LoadedPage>,
    next: Option<&LoadedPage>,
    next_loading: bool,
    col: usize,
  ) -> usize {
    let mut count = self.standalone_lines.len();

    // A head partial inherited from the previous page's tail is stitched into
    // that page's seam, so `flat_lines` never re-emits it here. Drop it up
    // front — before the image early-return below — so the count stays in
    // lock-step with `flat_lines` even when the next page carries images.
    if let Some(head) = &self.head_partial
      && prev.is_some_and(|p| p.tail_partial.is_some())
    {
      count = count.saturating_sub(head.line_count);
    }

    if self.contains_images || next.is_some_and(|p| p.contains_images) {
      if next.is_some() || next_loading {
        count += 1;
      }
      return count.max(1);
    }

    let emitted_seam = self.tail_partial.is_some()
      && next.is_some_and(|n| n.head_partial.is_some());
    if let Some(tail) = &self.tail_partial
      && let Some(next_page) = next
      && let Some(next_head) = next_page.head_partial.as_ref()
    {
      count = count.saturating_sub(tail.line_count);
      let seam = justify_pdf_seam(&tail.raw_text, &next_head.raw_text, col);
      count += seam.len();
    }

    // Inter-page separator. With edge blanks trimmed in
    // `justify_pdf_page`, every page's standalone_lines starts and ends
    // with content, so `flat_lines` is the one place that decides how
    // many blanks sit between two adjacent pages. Mirror that decision
    // here so summed per-page counts stay in lock-step with
    // `flat_lines.len()` — otherwise `line_start_for_page` walks the
    // cursor to the wrong row whenever a streamed PDF page boundary
    // crosses a list / caption continuation.
    if !emitted_seam {
      if let Some(next_page) = next {
        count += inter_page_blank_count(
          &self.standalone_lines,
          &next_page.standalone_lines,
        );
      } else if next_loading {
        // Default to one separator when the next page hasn't loaded —
        // matches the placeholder spacing in `flat_lines`.
        count += 1;
      }
    }
    count.max(1)
  }
}

impl LoadedPage {
  pub fn from_raw(raw_text: String, col: usize) -> Self {
    let PdfPageJustified { lines, head_partial, tail_partial } =
      justify_pdf_page(&raw_text, col);
    let line_kinds = vec![PdfLineKind::Text; lines.len()];
    Self {
      raw_text,
      standalone_lines: lines,
      line_kinds,
      contains_images: false,
      ocr_enhanced: false,
      head_partial: head_partial.map(Into::into),
      tail_partial: tail_partial.map(Into::into),
    }
  }

  pub fn from_rendered(page: PdfRenderedPage, col: usize) -> Self {
    if !page.contains_images {
      return Self::from_raw(page.raw_text, col);
    }

    let mut line_kinds = page.line_kinds;
    if line_kinds.len() != page.lines.len() {
      line_kinds = vec![PdfLineKind::Text; page.lines.len()];
    }

    Self {
      raw_text: page.raw_text,
      standalone_lines: page.lines,
      line_kinds,
      contains_images: true,
      ocr_enhanced: false,
      head_partial: None,
      tail_partial: None,
    }
  }
}
