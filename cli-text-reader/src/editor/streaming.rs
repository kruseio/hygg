use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Receiver;

use cli_justify::{
  PartialParagraph, PdfPageJustified, inter_page_blank_count, justify_pdf_page,
  justify_pdf_seam,
};
use cli_pdf_to_text::SharedPdfStream;

/// Lines reserved for a Loading slot in the flat editor buffer.
pub const PLACEHOLDER_LINES_PER_PAGE: usize = 1;

#[derive(Clone)]
pub enum PageSlot {
  Loading,
  Loaded(LoadedPage),
}

#[derive(Clone)]
pub struct LoadedPage {
  /// Raw sanitized page text as returned by `PdfStream::extract_page`.
  /// Retained so the page can be re-justified on column-width changes
  /// without re-running PDF extraction.
  #[allow(dead_code)]
  pub raw_text: String,
  /// Standalone justified output for the page. Kept immutable so seam
  /// stitching can be recomputed idempotently as neighbours load.
  pub standalone_lines: Vec<String>,
  /// Leading partial paragraph (if it looks like a continuation from the
  /// previous page).
  pub head_partial: Option<StoredPartial>,
  /// Trailing partial paragraph (if it looks incomplete and may continue
  /// onto the next page).
  pub tail_partial: Option<StoredPartial>,
}

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

    if let Some(head) = &self.head_partial
      && prev.is_some_and(|p| p.tail_partial.is_some())
    {
      count = count.saturating_sub(head.line_count);
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
        count +=
          inter_page_blank_count(&self.standalone_lines, &next_page.standalone_lines);
      } else if next_loading {
        // Default to one separator when the next page hasn't loaded —
        // matches the placeholder spacing in `flat_lines`.
        count += 1;
      }
    }
    count.max(1)
  }
}

#[derive(Clone)]
pub struct StoredPartial {
  pub raw_text: String,
  pub line_count: usize,
}

impl From<PartialParagraph> for StoredPartial {
  fn from(p: PartialParagraph) -> Self {
    Self { raw_text: p.raw_text, line_count: p.line_count }
  }
}

impl LoadedPage {
  pub fn from_raw(raw_text: String, col: usize) -> Self {
    let PdfPageJustified { lines, head_partial, tail_partial } =
      justify_pdf_page(&raw_text, col);
    Self {
      raw_text,
      standalone_lines: lines,
      head_partial: head_partial.map(Into::into),
      tail_partial: tail_partial.map(Into::into),
    }
  }
}

impl PageSlot {
  pub fn is_loaded(&self) -> bool {
    matches!(self, PageSlot::Loaded(_))
  }

  pub fn as_loaded(&self) -> Option<&LoadedPage> {
    if let PageSlot::Loaded(p) = self { Some(p) } else { None }
  }
}

/// Message posted by the background loader when it finishes extracting one
/// page's raw text. Justification happens on the main thread when the
/// message is drained.
pub struct PageLoaded {
  pub page_index: usize,
  pub raw_text: String,
}

/// Result of the background "open the PDF + extract initial pages" job.
/// Sent once when the synchronous open + preload finish; afterwards the
/// loader thread continues with page-by-page extraction.
pub enum StreamReady {
  Ok {
    stream: SharedPdfStream,
    target_page: usize,
    /// Raw page text for every page extracted by the opener thread before
    /// the editor's first render. Includes the target page and a window
    /// of neighbours so the viewport is stable from the very first frame.
    /// Entries are `(page_1based, raw_text)`.
    preloaded_pages: Vec<(usize, String)>,
    pages_receiver: Receiver<PageLoaded>,
    cancel: Arc<AtomicBool>,
    worker: std::thread::JoinHandle<()>,
  },
  Err(String),
}

/// Held by the editor while the PDF is being opened in the background.
pub struct PendingPdfStream {
  pub receiver: std::sync::mpsc::Receiver<StreamReady>,
  /// When the open job was kicked off — surfaced as elapsed time in the
  /// loading splash so the user can see hygg hasn't frozen on a slow open.
  pub started_at: std::time::Instant,
  /// Display-friendly path so the splash can show *which* file is opening.
  pub canonical_path_display: String,
  /// Saved cursor row within the target page's rendered output, if any.
  /// Restored as cursor position once the preloaded pages are installed.
  pub restore_line_in_page: Option<usize>,
}

pub struct PdfStreamingState {
  /// Held so the underlying parsed document outlives the loader thread and
  /// stays available for on-demand re-extraction.
  #[allow(dead_code)]
  pub stream: SharedPdfStream,
  pub col: usize,
  pub pages: Vec<PageSlot>,
  pub receiver: Receiver<PageLoaded>,
  /// Signals the background loader to stop. Flipped on editor exit.
  pub cancel: Arc<AtomicBool>,
  /// True once every page has been received and stitched.
  pub fully_loaded: bool,
  /// Worker thread join handle; held so the thread is cleanly joined when
  /// the editor exits.
  pub worker: Option<std::thread::JoinHandle<()>>,
}

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
    let next_loading =
      matches!(next_slot, Some(PageSlot::Loading));
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
