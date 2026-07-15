use super::{
  Editor, PDF_BUFFER_INDEX, PdfCursorAnchor, page_and_offset_for_line,
  reanchored_pdf_line, restored_pdf_viewport,
};
use crate::editor::streaming::{LoadedPage, PageSlot};
use cli_pdf_to_text::PdfLineKind;

impl Editor {
  /// Apply the pending streaming-PDF resume target *once the target page has
  /// real content*. Until then this is a no-op, so the saved position survives
  /// a placeholder's 1-line height — the row would clamp to 0, and the word
  /// anchor would "resolve" against the placeholder's own characters and land
  /// at the page start (the bundled-OCR resume bug: nothing preloads, so the
  /// target page is always a placeholder at install). Clears the target after
  /// applying so later page loads are governed only by the sticky re-anchoring
  /// in `drain_pdf_stream`.
  pub(crate) fn apply_pdf_restore_target_if_ready(&mut self) {
    let Some(target) = self.pdf_restore_target else {
      return;
    };
    let Some(state) = self.pdf_streaming.as_ref() else {
      return;
    };
    let idx = (target.page as usize).saturating_sub(1);
    // Wait for the page *and its immediate neighbours*: the anchor was saved
    // against the seam-stitched rendering, and a page's own slice shifts by
    // its head-partial rows until the previous page loads.
    if !state.page_render_settled(idx) {
      return;
    }
    let line_start = state.line_start_for_page(idx);
    let page_lines = state.page_line_count(idx);
    // The page is loaded: resolve the exact row now. The width-independent
    // word anchor wins; the saved row (clamped to the page's real height) is
    // the fallback for saves without one.
    let cursor_y_hint = target.cursor_y;
    let line_in_page = match target.word_offset {
      Some(word) => crate::word_anchor::line_for_word_in_range(
        &self.lines,
        &self.line_kinds,
        line_start,
        line_start + page_lines,
        word,
      ),
      None => target.line_in_page.min(page_lines.saturating_sub(1)),
    };
    let document_line = line_start + line_in_page;
    let content_height = self.height.saturating_sub(1);
    let (offset, cursor_y) =
      restored_pdf_viewport(document_line, content_height, cursor_y_hint);
    if let Some(buffer) = self.buffers.get_mut(PDF_BUFFER_INDEX) {
      buffer.offset = offset;
      buffer.cursor_y = cursor_y;
    }
    if self.active_buffer == PDF_BUFFER_INDEX {
      self.offset = offset;
      self.cursor_y = cursor_y;
      self.last_offset = document_line;
      self.last_saved_viewport_offset = self.offset;
    }
    self.pdf_restore_target = None;
    self.needs_redraw = true;
  }

  pub(crate) fn pdf_cursor_anchor(&self) -> Option<PdfCursorAnchor> {
    let state = self.pdf_streaming.as_ref()?;
    if state.pages.is_empty() {
      return None;
    }
    let (offset, cursor_y) = if self.active_buffer == PDF_BUFFER_INDEX {
      (self.offset, self.cursor_y)
    } else {
      self
        .buffers
        .get(PDF_BUFFER_INDEX)
        .map(|buffer| (buffer.offset, buffer.cursor_y))
        .unwrap_or((0, 0))
    };
    let (page_index, line_in_page) =
      page_and_offset_for_line(state, offset + cursor_y);
    Some(PdfCursorAnchor { page_index, line_in_page, screen_row: cursor_y })
  }

  pub(crate) fn apply_pdf_cursor_anchor(
    &mut self,
    page_counts: &[usize],
    anchor: PdfCursorAnchor,
  ) {
    let new_line = reanchored_pdf_line(page_counts, anchor);
    let offset = new_line.saturating_sub(anchor.screen_row);
    let cursor_y = new_line - offset;

    if let Some(buffer) = self.buffers.get_mut(PDF_BUFFER_INDEX) {
      buffer.offset = offset;
      buffer.cursor_y = cursor_y;
    }

    if self.active_buffer == PDF_BUFFER_INDEX {
      self.offset = offset;
      self.cursor_y = cursor_y;
      self.last_offset = new_line;
      self.last_saved_viewport_offset = self.offset;
    }
  }

  /// Rebuild `self.lines` (and total_lines / active buffer state) from the
  /// current PDF streaming page table. Called whenever a Loading slot
  /// transitions to Loaded, or after a seam stitch. No-op for sessions that
  /// aren't streaming a PDF.
  pub fn rebuild_lines_from_pdf_stream(&mut self) {
    let Some(state) = self.pdf_streaming.as_ref() else {
      return;
    };
    let new_lines = state.flat_lines();
    let new_line_kinds = state.flat_line_kinds();
    if let Some(buffer) = self.buffers.get_mut(PDF_BUFFER_INDEX) {
      buffer.lines = new_lines.clone();
      buffer.line_kinds = new_line_kinds.clone();
    }
    if self.active_buffer == PDF_BUFFER_INDEX {
      self.lines = new_lines;
      self.line_kinds = new_line_kinds;
      self.total_lines = self.lines.len();
    }
    self.needs_redraw = true;
  }

  pub fn start_pdf_ocr_loader(&mut self) -> bool {
    let Some(pdf_path) = self.pdf_source_path.clone() else {
      return false;
    };
    let start_page =
      self.current_pdf_buffer_position().map(|(p, _)| p as usize).unwrap_or(1);
    let Some(state) = self.pdf_streaming.as_mut() else {
      return false;
    };
    if state.ocr_loading {
      return false;
    }

    let total_pages = state.pages.len();
    if total_pages == 0 {
      return false;
    }
    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (receiver, worker) = crate::editor::streaming_loader::spawn_ocr_loader(
      pdf_path,
      start_page,
      state.col,
      total_pages,
      std::sync::Arc::clone(&cancel),
    );
    state.ocr_receiver = Some(receiver);
    state.ocr_cancel = Some(cancel);
    state.ocr_worker = Some(worker);
    state.ocr_loading = true;
    self.needs_redraw = true;
    true
  }

  pub fn stop_pdf_ocr_loader(&mut self) -> bool {
    let Some(state) = self.pdf_streaming.as_mut() else {
      return false;
    };
    let was_loading = state.ocr_loading;
    if let Some(cancel) = state.ocr_cancel.take() {
      cancel.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    state.ocr_receiver = None;
    state.ocr_worker = None;
    state.ocr_loading = false;
    self.needs_redraw = true;
    self.reload_ocr_enhanced_pdf_pages();
    was_loading
  }

  fn reload_ocr_enhanced_pdf_pages(&mut self) -> bool {
    let Some(anchor) = self.pdf_cursor_anchor() else {
      return false;
    };
    let Some(state) = self.pdf_streaming.as_mut() else {
      return false;
    };

    let stream = std::sync::Arc::clone(&state.stream);
    let col = state.col;
    let ocr_pages: Vec<usize> = state
      .pages
      .iter()
      .enumerate()
      .filter_map(|(idx, slot)| {
        slot.as_loaded().and_then(|page| page.ocr_enhanced.then_some(idx))
      })
      .collect();
    if ocr_pages.is_empty() {
      return false;
    }

    for idx in ocr_pages {
      if let Some(rendered_page) = stream.extract_page_with_images(idx + 1, col)
      {
        state.pages[idx] =
          PageSlot::Loaded(LoadedPage::from_rendered(rendered_page, col));
      }
    }
    let pages_snapshot: Vec<usize> =
      (0..state.pages.len()).map(|i| state.page_line_count(i)).collect();

    self.rebuild_lines_from_pdf_stream();
    self.apply_pdf_cursor_anchor(&pages_snapshot, anchor);
    self.needs_redraw = true;
    true
  }

  pub fn is_ansi_art_line(&self, line_idx: usize) -> bool {
    self
      .line_kinds
      .get(line_idx)
      .is_some_and(|kind| *kind == PdfLineKind::AnsiArt)
  }

  pub fn is_buffer_ansi_art_line(
    &self,
    buffer_idx: usize,
    line_idx: usize,
  ) -> bool {
    self
      .buffers
      .get(buffer_idx)
      .and_then(|buffer| buffer.line_kinds.get(line_idx))
      .is_some_and(|kind| *kind == PdfLineKind::AnsiArt)
  }
}
