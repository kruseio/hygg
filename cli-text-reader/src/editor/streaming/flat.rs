use cli_justify::{inter_page_blank_count, justify_pdf_seam};
use cli_pdf_to_text::PdfLineKind;

use super::types::{PLACEHOLDER_LINES_PER_PAGE, PageSlot, PdfStreamingState};

impl PdfStreamingState {
  pub fn placeholder_line(page_index: usize, total_pages: usize) -> String {
    format!("  [ loading page {} of {} … ]", page_index + 1, total_pages)
  }

  /// Build the flat lines view from the current page table, applying seam
  /// stitching between adjacent loaded pages.
  pub fn flat_lines(&self) -> Vec<String> {
    let total_pages = self.pages.len();
    let mut out: Vec<String> = Vec::new();
    // True when the most recently emitted lines were a seam that
    // stitches the previous page into the next — no separator should
    // be inserted in that case because the seam IS the connection.
    let mut last_emit_was_seam = false;
    for idx in 0..total_pages {
      // Insert the inter-page separator BEFORE pushing this page's
      // content. The amount is decided by the same `inter_page_blank_count`
      // that `rendered_line_count` uses, so per-page line counts stay
      // in sync with `out.len()`.
      if idx > 0 && !last_emit_was_seam {
        let separators = self.separator_before_page(idx);
        for _ in 0..separators {
          out.push(String::new());
        }
      }
      last_emit_was_seam = false;

      match &self.pages[idx] {
        PageSlot::Loading => {
          for _ in 0..PLACEHOLDER_LINES_PER_PAGE {
            out.push(Self::placeholder_line(idx, total_pages));
          }
        }
        PageSlot::Loaded(page) => {
          let prev =
            if idx == 0 { None } else { self.pages[idx - 1].as_loaded() };
          let next = self.pages.get(idx + 1).and_then(PageSlot::as_loaded);

          let head_skip = if let Some(head) = &page.head_partial
            && prev.is_some_and(|p| p.tail_partial.is_some())
          {
            head.line_count
          } else {
            0
          };

          let (tail_skip, seam_lines) = if let Some(tail) = &page.tail_partial
            && let Some(next_page) = next
            && let Some(next_head) = next_page.head_partial.as_ref()
          {
            let seam =
              justify_pdf_seam(&tail.raw_text, &next_head.raw_text, self.col);
            (tail.line_count, Some(seam))
          } else {
            (0, None)
          };

          let standalone = &page.standalone_lines;
          if !standalone.is_empty() {
            let end = standalone.len().saturating_sub(tail_skip);
            let start = head_skip.min(end);
            for line in &standalone[start..end] {
              out.push(line.clone());
            }
          }
          if let Some(seam) = seam_lines {
            for line in seam {
              out.push(line);
            }
            last_emit_was_seam = true;
          }
        }
      }
    }
    if out.is_empty() {
      out.push(String::new());
    }
    out
  }

  pub fn flat_line_kinds(&self) -> Vec<PdfLineKind> {
    let total_pages = self.pages.len();
    let mut out = Vec::new();
    let mut last_emit_was_seam = false;
    for idx in 0..total_pages {
      if idx > 0 && !last_emit_was_seam {
        let separators = self.separator_before_page(idx);
        for _ in 0..separators {
          out.push(PdfLineKind::Text);
        }
      }
      last_emit_was_seam = false;

      match &self.pages[idx] {
        PageSlot::Loading => {
          for _ in 0..PLACEHOLDER_LINES_PER_PAGE {
            out.push(PdfLineKind::Text);
          }
        }
        PageSlot::Loaded(page) => {
          let prev =
            if idx == 0 { None } else { self.pages[idx - 1].as_loaded() };
          let next = self.pages.get(idx + 1).and_then(PageSlot::as_loaded);

          let head_skip = if let Some(head) = &page.head_partial
            && prev.is_some_and(|p| p.tail_partial.is_some())
          {
            head.line_count
          } else {
            0
          };

          let (tail_skip, seam_lines) = if let Some(tail) = &page.tail_partial
            && let Some(next_page) = next
            && let Some(next_head) = next_page.head_partial.as_ref()
          {
            let seam =
              justify_pdf_seam(&tail.raw_text, &next_head.raw_text, self.col);
            (tail.line_count, Some(seam))
          } else {
            (0, None)
          };

          if !page.standalone_lines.is_empty() {
            let end = page.standalone_lines.len().saturating_sub(tail_skip);
            let start = head_skip.min(end);
            for kind in &page.line_kinds[start..end] {
              out.push(*kind);
            }
          }
          if let Some(seam) = seam_lines {
            out.extend(std::iter::repeat_n(PdfLineKind::Text, seam.len()));
            last_emit_was_seam = true;
          }
        }
      }
    }
    if out.is_empty() {
      out.push(PdfLineKind::Text);
    }
    out
  }

  /// Decide the number of separator blanks `flat_lines` should insert
  /// directly before page `idx`. Returns 0 when the prior page already
  /// emitted a seam into this one (the seam is the connection), or when
  /// the two pages share a sibling list / caption that should read
  /// continuously. Otherwise 1, the normal paragraph break.
  fn separator_before_page(&self, idx: usize) -> usize {
    if idx == 0 {
      return 0;
    }
    let prev_slot = &self.pages[idx - 1];
    let this_slot = &self.pages[idx];
    let prev_loaded = prev_slot.as_loaded();
    let this_loaded = this_slot.as_loaded();
    match (prev_loaded, this_loaded) {
      (Some(prev), Some(this)) => {
        if prev.contains_images || this.contains_images {
          return 1;
        }
        inter_page_blank_count(&prev.standalone_lines, &this.standalone_lines)
      }
      _ => 1,
    }
  }

  /// Number of flat lines a given page index will contribute, taking
  /// neighbour-driven stitching into account.
  pub fn page_line_count(&self, page_index: usize) -> usize {
    if page_index >= self.pages.len() {
      return 0;
    }
    let next_slot = self.pages.get(page_index + 1);
    let next_loading = matches!(next_slot, Some(PageSlot::Loading));
    match &self.pages[page_index] {
      PageSlot::Loading => {
        // A loading slot contributes its placeholder lines plus the
        // 1-blank default separator before the next page (matching
        // what `flat_lines` will emit). The separator is omitted when
        // there is no next page.
        let mut count = PLACEHOLDER_LINES_PER_PAGE;
        if next_slot.is_some() {
          count += 1;
        }
        count
      }
      PageSlot::Loaded(page) => {
        let prev = if page_index == 0 {
          None
        } else {
          self.pages[page_index - 1].as_loaded()
        };
        let next = next_slot.and_then(PageSlot::as_loaded);
        page.rendered_line_count(prev, next, next_loading, self.col)
      }
    }
  }

  /// Sum of `page_line_count()` across all pages up to (not including)
  /// `page_index`.
  pub fn line_start_for_page(&self, page_index: usize) -> usize {
    let upto = page_index.min(self.pages.len());
    (0..upto).map(|i| self.page_line_count(i)).sum()
  }
}
