use super::{Editor, restored_pdf_viewport};
use cli_pdf_to_text::PdfLineKind;

mod query;

impl Editor {
  /// Poll the background "open PDF" thread; if it has finished, install
  /// the resulting streaming state (or surface the error in the editor
  /// buffer). Returns true if state changed.
  pub fn poll_pending_pdf_stream(&mut self) -> bool {
    use crate::editor::streaming::{
      LoadedPage, PageSlot, PdfStreamingState, StreamReady,
    };
    let Some(pending) = self.pdf_pending.as_ref() else {
      return false;
    };
    let message = match pending.receiver.try_recv() {
      Ok(msg) => msg,
      Err(std::sync::mpsc::TryRecvError::Empty) => {
        return false;
      }
      Err(std::sync::mpsc::TryRecvError::Disconnected) => {
        // Open thread died without sending — surface a generic error.
        self.lines = vec![
          "  Failed to open PDF (background opener exited unexpectedly)."
            .into(),
        ];
        self.line_kinds = vec![PdfLineKind::Text; self.lines.len()];
        self.total_lines = self.lines.len();
        if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
          buffer.lines = self.lines.clone();
          buffer.line_kinds = self.line_kinds.clone();
        }
        self.pdf_pending = None;
        self.needs_redraw = true;
        return true;
      }
    };
    let restore_line_in_page =
      self.pdf_pending.as_ref().and_then(|p| p.restore_line_in_page);
    let restore_word_offset =
      self.pdf_pending.as_ref().and_then(|p| p.restore_word_offset);
    let restore_cursor_y =
      self.pdf_pending.as_ref().and_then(|p| p.restore_cursor_y);
    let pending_info = self.pdf_pending.as_ref().map(|p| {
      let filename = std::path::Path::new(&p.canonical_path_display)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&p.canonical_path_display)
        .to_string();
      (p.started_at, filename)
    });
    self.pdf_pending = None;
    match message {
      StreamReady::Err(err) => {
        self.lines = vec![format!("  {err}")];
        self.line_kinds = vec![PdfLineKind::Text; self.lines.len()];
        self.total_lines = self.lines.len();
        if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
          buffer.lines = self.lines.clone();
          buffer.line_kinds = self.line_kinds.clone();
        }
        self.needs_redraw = true;
      }
      StreamReady::Ok {
        stream,
        target_page,
        restore_line_in_page: ready_restore_line_in_page,
        preloaded_pages,
        pages_receiver,
        cancel,
        worker,
        ocr_loading,
      } => {
        let total_pages = stream.total_pages();
        let mut pages: Vec<PageSlot> =
          (0..total_pages).map(|_| PageSlot::Loading).collect();
        for (page_1based, rendered_page) in preloaded_pages {
          if page_1based == 0 || page_1based > total_pages {
            continue;
          }
          let loaded = LoadedPage::from_rendered(rendered_page, self.col);
          pages[page_1based - 1] = PageSlot::Loaded(loaded);
        }

        let fully_loaded = pages.iter().all(|p| p.is_loaded());
        let state = PdfStreamingState {
          stream,
          col: self.col,
          pages,
          receiver: pages_receiver,
          cancel,
          fully_loaded,
          ocr_loading,
          ocr_receiver: None,
          ocr_cancel: None,
          ocr_worker: None,
          worker: Some(worker),
        };
        let target_line_start = state.line_start_for_page(target_page - 1);
        let target_page_lines = state.page_line_count(target_page - 1);
        self.pdf_streaming = Some(state);
        if self.ocr_enabled {
          self.start_pdf_ocr_loader();
        }
        self.rebuild_lines_from_pdf_stream();
        // Provisionally land at the saved row, clamped to the target page's
        // *current* rendered height. This is only the placement while the page
        // is a placeholder; the exact row — preferring the width-independent
        // word anchor — is resolved by `apply_pdf_restore_target_if_ready`
        // below, and only once the page has real content. Resolving the anchor
        // here against a 1-line placeholder (e.g. bundled-OCR opens preload
        // nothing) would land at the page start and lose the position.
        let saved_line_in_page =
          ready_restore_line_in_page.or(restore_line_in_page).unwrap_or(0);
        let line_in_page =
          saved_line_in_page.min(target_page_lines.saturating_sub(1));
        let document_line = target_line_start + line_in_page;
        // Place the cursor on the same screen row the splash used so the
        // visible cursor / highlight bar doesn't shift when streaming
        // state takes over. center_cursor() on the next render is then a
        // no-op for the common case; edge case (document_line < center_y)
        // falls back to clamping near the top, matching center_cursor's
        // overscroll handling.
        let content_height = self.height.saturating_sub(1);
        let (offset, cursor_y) = restored_pdf_viewport(
          document_line,
          content_height,
          restore_cursor_y,
        );
        self.offset = offset;
        self.cursor_y = cursor_y;
        self.last_offset = document_line;
        self.last_saved_viewport_offset = self.offset;
        self.needs_redraw = true;
        // The placement above clamps the saved row against whatever the target
        // page measures *now*. When the page hasn't preloaded (a placeholder),
        // that clamps the row to 0 and loses the position. Record the unclamped
        // resume target — with the exact word anchor still unresolved — and
        // (re)apply it the moment the page has real content: here if it
        // preloaded, otherwise from `drain_pdf_stream`.
        self.pdf_restore_target = Some(crate::core_state::PdfRestoreTarget {
          page: target_page as u32,
          line_in_page: saved_line_in_page,
          cursor_y: restore_cursor_y,
          word_offset: restore_word_offset,
        });
        self.apply_pdf_restore_target_if_ready();
        if fully_loaded {
          if let Some((started, name)) = pending_info {
            self.pdf_load_finished = Some((
              std::time::Instant::now(),
              started.elapsed().as_secs_f32(),
              name,
            ));
          }
        } else if let Some(info) = pending_info {
          self.pdf_load_started_at = Some(info);
        }
        // A server-progress jump may have arrived while the splash was up and
        // been deferred (no page table to anchor against). Now that the page
        // table exists, apply it page-aware (or surface the prompt). This
        // overrides the local restore above only when the server row is newer.
        self.resolve_pending_server_progress_after_install();
      }
    }
    true
  }

  /// Drain any pages the background loader has finished extracting and
  /// install them into the page table. Returns the number of pages that
  /// were newly applied (0 if the channel was empty). Maintains viewport
  /// stickiness: after rebuilding the flat lines, the cursor stays on the
  /// same (page, line-within-page) it was on before the drain.
  pub fn drain_pdf_stream(&mut self) -> usize {
    use crate::editor::streaming::{LoadedPage, PageLoaded, PageSlot};
    let ocr_enabled = self.ocr_enabled;
    // Collect messages in a tight loop to avoid mutable-borrow churn.
    let messages = {
      let Some(state) = self.pdf_streaming.as_mut() else {
        return 0;
      };
      let mut messages: Vec<_> = state.receiver.try_iter().collect();
      if let Some(receiver) = state.ocr_receiver.as_ref() {
        messages.extend(receiver.try_iter());
      }
      messages
    };
    if messages.is_empty() {
      return 0;
    }

    // Snapshot the logical cursor location: which page, which row within
    // that page's flat-lines slice.
    let Some(anchor) = self.pdf_cursor_anchor() else {
      return 0;
    };
    let Some(state) = self.pdf_streaming.as_mut() else {
      return 0;
    };

    let col = state.col;
    let mut applied = 0usize;
    for msg in messages {
      let PageLoaded::Page { page_index: idx, rendered_page, replace_existing } =
        msg
      else {
        state.ocr_loading = false;
        state.ocr_receiver = None;
        state.ocr_cancel = None;
        if let Some(worker) = state.ocr_worker.take() {
          let _ = worker.join();
        }
        applied += 1;
        continue;
      };
      if idx >= state.pages.len() {
        continue;
      }
      if !replace_existing && let PageSlot::Loaded(_) = state.pages[idx] {
        continue;
      }
      let mut loaded = LoadedPage::from_rendered(rendered_page, col);
      loaded.ocr_enhanced = replace_existing && ocr_enabled;
      state.pages[idx] = PageSlot::Loaded(loaded);
      applied += 1;
    }
    if applied == 0 {
      return 0;
    }
    state.fully_loaded = state.pages.iter().all(|p| p.is_loaded());
    let just_finished = state.fully_loaded;

    // Snapshot per-page line counts AFTER applying the swaps. Used below
    // to re-anchor the viewport on the same (page, line-in-page) the
    // cursor was on prior to the swap.
    let pages_snapshot: Vec<usize> =
      (0..state.pages.len()).map(|i| state.page_line_count(i)).collect();

    if just_finished
      && let Some((started, name)) = self.pdf_load_started_at.take()
    {
      self.pdf_load_finished = Some((
        std::time::Instant::now(),
        started.elapsed().as_secs_f32(),
        name,
      ));
    }

    self.rebuild_lines_from_pdf_stream();

    // Re-anchor the viewport: keep the same PDF page/line on the same
    // screen row it was previously occupying.
    self.apply_pdf_cursor_anchor(&pages_snapshot, anchor);
    // If a resume target is still pending and its page just gained real
    // content, land on the exact saved row now (overrides the placeholder
    // anchoring above). No-op once applied or when the user has navigated.
    self.apply_pdf_restore_target_if_ready();
    self.needs_redraw = true;
    applied
  }
}
