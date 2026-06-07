pub use crate::core_state::Editor;
pub use crate::core_types::{
  BufferState, EditorMode, EditorState, SplitPosition, ViewMode,
};

use crate::editor::streaming::{LoadedPage, PageSlot, PdfStreamingState};
use crate::highlights::HighlightData;
use crate::progress::generate_hash;
use arboard::Clipboard;
use cli_pdf_to_text::PdfLineKind;
use crossterm::terminal;

const PDF_BUFFER_INDEX: usize = 0;

#[derive(Clone, Copy)]
struct PdfCursorAnchor {
  page_index: usize,
  line_in_page: usize,
  screen_row: usize,
}

/// Map a flat line index back to (page_index, line_within_page) using the
/// streaming state's current per-page rendered line counts.
fn page_and_offset_for_line(
  state: &PdfStreamingState,
  line: usize,
) -> (usize, usize) {
  let mut accumulated = 0usize;
  for idx in 0..state.pages.len() {
    let count = state.page_line_count(idx);
    if line < accumulated + count {
      return (idx, line - accumulated);
    }
    accumulated += count;
  }
  let last_idx = state.pages.len().saturating_sub(1);
  let last_count = state.page_line_count(last_idx);
  (last_idx, last_count.saturating_sub(1))
}

fn reanchored_pdf_line(
  page_counts: &[usize],
  anchor: PdfCursorAnchor,
) -> usize {
  let mut line = 0usize;
  for (idx, count) in page_counts.iter().enumerate() {
    if idx >= anchor.page_index {
      break;
    }
    line += count;
  }
  let clamped_line_in_page = anchor.line_in_page.min(
    page_counts.get(anchor.page_index).copied().unwrap_or(0).saturating_sub(1),
  );
  line + clamped_line_in_page
}

fn restored_pdf_viewport(
  document_line: usize,
  content_height: usize,
  restore_cursor_y: Option<usize>,
) -> (usize, usize) {
  let landing_y = restore_cursor_y
    .unwrap_or(content_height / 2)
    .min(content_height.saturating_sub(1));
  if document_line < landing_y {
    (0, document_line)
  } else {
    (document_line - landing_y, landing_y)
  }
}

impl Editor {
  pub fn new(lines: Vec<String>, col: usize) -> Self {
    Self::new_internal(lines, col, None)
  }

  pub fn new_with_content(
    lines: Vec<String>,
    col: usize,
    raw_content: String,
  ) -> Self {
    Self::new_internal(lines, col, Some(raw_content))
  }

  fn new_internal(
    lines: Vec<String>,
    col: usize,
    raw_content: Option<String>,
  ) -> Self {
    crate::debug::debug_log("editor", "Creating new Editor instance");

    // Generate hash from raw content if provided, otherwise from lines
    let document_hash = if let Some(content) = &raw_content {
      crate::debug::debug_log("editor", "Generating hash from raw content");
      generate_hash(content)
    } else {
      crate::debug::debug_log("editor", "Generating hash from justified lines");
      generate_hash(&lines)
    };

    let total_lines = lines.len();
    let (width, height) = terminal::size()
      .map(|(w, h)| (w as usize, h as usize))
      .unwrap_or((80, 24));

    // Startup narration voice + speed (env / .env / af_heart default); the
    // `:voice` and `:speed` commands mutate these live.
    let (tts_voice, tts_speed) = crate::config::tts_settings();

    crate::debug::debug_log_state(
      "editor",
      "document_hash",
      &document_hash.to_string(),
    );
    crate::debug::debug_log_state(
      "editor",
      "total_lines",
      &total_lines.to_string(),
    );
    crate::debug::debug_log_state(
      "editor",
      "terminal_size",
      &format!("{width}x{height}"),
    );

    // Initialize clipboard - may fail on headless systems
    let clipboard = Clipboard::new().ok();
    crate::debug::debug_log_state(
      "editor",
      "clipboard_available",
      &clipboard.is_some().to_string(),
    );

    // Create initial buffer with the document
    let mut initial_buffer = BufferState::new(lines.clone());
    initial_buffer.viewport_height = height.saturating_sub(1);
    initial_buffer.viewport_start = 0;

    crate::debug::debug_log("editor", "Editor instance created successfully");

    Self {
      lines,
      line_kinds: vec![PdfLineKind::Text; total_lines],
      col,
      offset: 0,
      width,
      height,
      show_highlighter: true,
      editor_state: EditorState::new(),
      document_hash,
      total_lines,
      progress_display_until: None,
      show_progress: false,
      cursor_x: 0,
      cursor_y: height / 2,
      clipboard,
      buffers: vec![initial_buffer],
      active_buffer: 0,
      view_mode: ViewMode::Normal,
      show_cursor: true,
      last_find_char: None,
      last_find_forward: true,
      last_find_till: false,
      marks: std::collections::HashMap::new(),
      previous_position: None,
      number_prefix: String::new(),
      highlights: HighlightData::new(document_hash.to_string()),
      active_pane: 0,
      split_ratio: 0.7, // 70% for main buffer, 30% for command output
      tmux_prefix_active: false,
      needs_redraw: true,
      last_offset: 0,
      force_clear: true,
      cursor_moved: false,
      tutorial_step: 0,
      tutorial_active: false,
      tutorial_demo_mode: false,
      tutorial_start_time: None,
      demo_script: None,
      demo_action_index: 0,
      demo_id: None,
      demo_last_action_time: None,
      demo_hint_text: None,
      demo_hint_until: None,
      demo_typing_char_index: 0,
      demo_pending_keys: Vec::new(),
      current_tutorial_condition: None,
      tutorial_highlight_created: false,
      tutorial_yank_performed: false,
      tutorial_paste_performed: false,
      tutorial_search_navigated: false,
      tutorial_bookmark_jumped: false,
      tutorial_forward_search_used: false,
      tutorial_backward_search_used: false,
      last_executed_command: None,
      tutorial_step_completed: false,
      initial_setup_complete: false,
      last_saved_viewport_offset: 0,
      cursor_currently_visible: true,
      last_cursor_style: None,
      buffer_just_switched: false,
      pdf_streaming: None,
      pdf_source_path: None,
      ocr_enabled: false,
      pdf_pending: None,
      pdf_load_started_at: None,
      pdf_load_finished: None,
      speech: None,
      tts_voice,
      tts_speed,
    }
  }

  // Get the actual cursor position in the document (line_index, column)
  pub fn get_cursor_position(&self) -> (usize, usize) {
    // Calculate the correct line index based on the cursor's position
    // This ensures we get the line currently being displayed under the cursor
    let line_idx = self.offset + self.cursor_y;

    // Make sure we don't exceed the document boundaries
    let line_idx = line_idx.min(self.lines.len().saturating_sub(1));

    (line_idx, self.cursor_x)
  }

  // Debug logging helper
  pub fn debug_log(&self, message: &str) {
    crate::debug::debug_log("editor", message);
  }

  pub fn debug_log_event(&self, module: &str, event: &str, details: &str) {
    crate::debug::debug_log_event(module, event, details);
  }

  pub fn debug_log_state(
    &self,
    module: &str,
    state_name: &str,
    state_value: &str,
  ) {
    crate::debug::debug_log_state(module, state_name, state_value);
  }

  pub fn debug_log_error(&self, error: &str) {
    crate::debug::debug_log_error("editor", error);
  }

  // Calculate dimensions for display
  #[allow(dead_code)]
  pub fn calculate_dimensions(&self) -> usize {
    // Always use full height minus status line
    self.height.saturating_sub(1)
  }

  // Helper methods to access active buffer's mode and command state
  pub fn get_active_mode(&self) -> EditorMode {
    if let Some(buffer) = self.buffers.get(self.active_buffer) {
      buffer.mode.clone()
    } else {
      // Fallback to editor state mode during migration
      self.editor_state.mode.clone()
    }
  }

  pub fn set_active_mode(&mut self, mode: EditorMode) {
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.mode = mode.clone();
    }
    // Also update editor state during migration
    self.editor_state.mode = mode;
  }

  pub fn get_active_command_buffer(&self) -> &str {
    if let Some(buffer) = self.buffers.get(self.active_buffer) {
      &buffer.command_buffer
    } else {
      // Fallback to editor state during migration
      &self.editor_state.command_buffer
    }
  }

  #[allow(dead_code)]
  pub fn get_active_command_buffer_mut(&mut self) -> &mut String {
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      &mut buffer.command_buffer
    } else {
      // Fallback to editor state during migration
      &mut self.editor_state.command_buffer
    }
  }

  pub fn get_active_command_cursor_pos(&self) -> usize {
    if let Some(buffer) = self.buffers.get(self.active_buffer) {
      buffer.command_cursor_pos
    } else {
      // Fallback to editor state during migration
      self.editor_state.command_cursor_pos
    }
  }

  #[allow(dead_code)]
  pub fn set_active_command_cursor_pos(&mut self, pos: usize) {
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.command_cursor_pos = pos;
    }
    // Also update editor state during migration
    self.editor_state.command_cursor_pos = pos;
  }

  // Save bookmarks to file
  pub fn save_bookmarks(&self) {
    use crate::bookmarks::save_bookmarks;
    if let Err(e) = save_bookmarks(self.document_hash, &self.marks) {
      self.debug_log_error(&format!("Failed to save bookmarks: {e}"));
    }
  }

  // Save highlights to file
  pub fn save_highlights(&self) {
    use crate::highlights::save_highlights;
    if let Err(e) = save_highlights(&self.highlights) {
      self.debug_log_error(&format!("Failed to save highlights: {e}"));
    }
  }

  // Mark editor as needing redraw
  pub fn mark_dirty(&mut self) {
    self.needs_redraw = true;
  }

  // Check if redraw is needed and reset flag
  pub fn check_needs_redraw(&mut self) -> bool {
    let needs = self.needs_redraw;
    self.needs_redraw = false;
    needs
  }

  fn pdf_cursor_anchor(&self) -> Option<PdfCursorAnchor> {
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

  fn apply_pdf_cursor_anchor(
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
        // Land at the saved row within the target page; clamp to the page's
        // current rendered height so a shrunk page or missing line_in_page
        // still produces a valid position.
        let line_in_page = restore_line_in_page
          .unwrap_or(0)
          .min(target_page_lines.saturating_sub(1));
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
    self.needs_redraw = true;
    applied
  }

  /// Return `(page_1based, line_in_page)` for the cursor's current position
  /// in a streaming PDF session, where `line_in_page` is the row within the
  /// page's rendered output. Returns None if not streaming a PDF.
  pub fn current_pdf_position(&self) -> Option<(u32, usize)> {
    let state = self.pdf_streaming.as_ref()?;
    if state.pages.is_empty() {
      return None;
    }
    let target_line = self.offset + self.cursor_y;
    let (page_idx, line_in_page) = page_and_offset_for_line(state, target_line);
    Some(((page_idx + 1) as u32, line_in_page))
  }

  fn current_pdf_buffer_position(&self) -> Option<(u32, usize)> {
    let anchor = self.pdf_cursor_anchor()?;
    Some(((anchor.page_index + 1) as u32, anchor.line_in_page))
  }

  // Get the effective viewport height for the current buffer
  pub fn get_effective_viewport_height(&self) -> usize {
    if self.view_mode == ViewMode::HorizontalSplit {
      if let Some(buffer) = self.buffers.get(self.active_buffer) {
        buffer.viewport_height
      } else {
        self.height.saturating_sub(1)
      }
    } else {
      self.height.saturating_sub(1)
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::editor::streaming::{
    LoadedPage, PageLoaded, PageSlot, PdfStreamingState,
  };
  use cli_pdf_to_text::{PdfRenderedPage, PdfStream};
  use std::sync::atomic::AtomicBool;
  use std::sync::{Arc, mpsc};

  fn test_pdf_stream() -> Option<Arc<PdfStream>> {
    let pdf_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("../test-data/pdf/progit-1-50.pdf");
    if !pdf_path.exists() {
      return None;
    }
    Some(Arc::new(
      PdfStream::open(pdf_path.to_str().expect("utf-8 path"))
        .expect("PdfStream should open valid test PDF"),
    ))
  }

  fn rendered_image_page(lines: &[&str]) -> PdfRenderedPage {
    PdfRenderedPage {
      raw_text: lines.join("\n"),
      lines: lines.iter().map(|line| (*line).to_string()).collect(),
      line_kinds: vec![PdfLineKind::Text; lines.len()],
      contains_images: true,
    }
  }

  fn editor_with_two_page_pdf() -> Option<(Editor, mpsc::SyncSender<PageLoaded>)>
  {
    let stream = test_pdf_stream()?;
    let (tx, rx) = mpsc::sync_channel(4);
    let mut editor = Editor::new(vec!["placeholder".to_string()], 80);
    editor.height = 10;
    editor.ocr_enabled = true;
    editor.pdf_streaming = Some(PdfStreamingState {
      stream,
      col: 80,
      pages: vec![
        PageSlot::Loaded(LoadedPage::from_rendered(
          rendered_image_page(&["p1-0", "p1-1", "p1-2"]),
          80,
        )),
        PageSlot::Loaded(LoadedPage::from_rendered(
          rendered_image_page(&["p2-0", "p2-1"]),
          80,
        )),
      ],
      receiver: rx,
      cancel: Arc::new(AtomicBool::new(false)),
      fully_loaded: true,
      ocr_loading: true,
      ocr_receiver: None,
      ocr_cancel: None,
      ocr_worker: None,
      worker: None,
    });
    editor.rebuild_lines_from_pdf_stream();
    Some((editor, tx))
  }

  #[test]
  fn pdf_restore_uses_saved_cursor_screen_row() {
    assert_eq!(restored_pdf_viewport(42, 24, Some(14)), (28, 14));
  }

  #[test]
  fn pdf_restore_clamps_saved_cursor_row_near_top() {
    assert_eq!(restored_pdf_viewport(3, 24, Some(14)), (0, 3));
  }

  #[test]
  fn pdf_restore_falls_back_to_center_without_saved_cursor_row() {
    assert_eq!(restored_pdf_viewport(42, 24, None), (30, 12));
  }

  #[test]
  fn drain_pdf_stream_replaces_loaded_page_for_ocr_update() {
    let pdf_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("../test-data/pdf/progit-1-50.pdf");
    if !pdf_path.exists() {
      return;
    }
    let stream = Arc::new(
      PdfStream::open(pdf_path.to_str().expect("utf-8 path"))
        .expect("PdfStream should open valid test PDF"),
    );
    let (tx, rx) = mpsc::sync_channel(1);
    let mut editor = Editor::new(vec!["fast page".to_string()], 80);
    editor.ocr_enabled = true;
    editor.pdf_streaming = Some(PdfStreamingState {
      stream,
      col: 80,
      pages: vec![PageSlot::Loaded(LoadedPage::from_raw(
        "fast page".to_string(),
        80,
      ))],
      receiver: rx,
      cancel: Arc::new(AtomicBool::new(false)),
      fully_loaded: true,
      ocr_loading: true,
      ocr_receiver: None,
      ocr_cancel: None,
      ocr_worker: None,
      worker: None,
    });
    editor.rebuild_lines_from_pdf_stream();

    tx.send(PageLoaded::Page {
      page_index: 0,
      rendered_page: PdfRenderedPage {
        raw_text: "ocr page".to_string(),
        lines: vec!["ocr page".to_string()],
        line_kinds: vec![PdfLineKind::Text],
        contains_images: true,
      },
      replace_existing: true,
    })
    .expect("replacement page should enqueue");

    assert_eq!(editor.drain_pdf_stream(), 1);
    assert_eq!(editor.lines, vec!["ocr page"]);
    let PageSlot::Loaded(page) =
      &editor.pdf_streaming.as_ref().expect("streaming state").pages[0]
    else {
      panic!("page should be loaded");
    };
    assert!(page.ocr_enhanced);
  }

  #[test]
  fn drain_pdf_stream_marks_ocr_complete() {
    let pdf_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("../test-data/pdf/progit-1-50.pdf");
    if !pdf_path.exists() {
      return;
    }
    let stream = Arc::new(
      PdfStream::open(pdf_path.to_str().expect("utf-8 path"))
        .expect("PdfStream should open valid test PDF"),
    );
    let (tx, rx) = mpsc::sync_channel(1);
    let mut editor = Editor::new(vec!["fast page".to_string()], 80);
    editor.pdf_streaming = Some(PdfStreamingState {
      stream,
      col: 80,
      pages: vec![PageSlot::Loaded(LoadedPage::from_raw(
        "fast page".to_string(),
        80,
      ))],
      receiver: rx,
      cancel: Arc::new(AtomicBool::new(false)),
      fully_loaded: true,
      ocr_loading: true,
      ocr_receiver: None,
      ocr_cancel: None,
      ocr_worker: None,
      worker: None,
    });

    tx.send(PageLoaded::OcrComplete).expect("OCR completion should enqueue");

    assert_eq!(editor.drain_pdf_stream(), 1);
    assert!(
      !editor.pdf_streaming.as_ref().expect("streaming state").ocr_loading
    );
  }

  #[test]
  fn ocr_replacement_preserves_page_line_and_cursor_screen_row() {
    let Some((mut editor, tx)) = editor_with_two_page_pdf() else {
      return;
    };
    editor.offset = 3;
    editor.cursor_y = 2;
    editor.buffers[0].offset = 3;
    editor.buffers[0].cursor_y = 2;
    assert_eq!(editor.current_pdf_position(), Some((2, 1)));

    tx.send(PageLoaded::Page {
      page_index: 0,
      rendered_page: rendered_image_page(&["p1 replacement"]),
      replace_existing: true,
    })
    .expect("replacement page should enqueue");

    assert_eq!(editor.drain_pdf_stream(), 1);
    assert_eq!(editor.current_pdf_position(), Some((2, 1)));
    assert_eq!(editor.cursor_y, 2);
    assert_eq!(editor.offset, 1);
  }

  #[test]
  fn drain_pdf_stream_updates_pdf_buffer_without_moving_overlay() {
    assert_non_pdf_active_drain_preserves_active_buffer(ViewMode::Overlay);
  }

  #[test]
  fn drain_pdf_stream_updates_pdf_buffer_without_moving_split() {
    assert_non_pdf_active_drain_preserves_active_buffer(
      ViewMode::HorizontalSplit,
    );
  }

  fn assert_non_pdf_active_drain_preserves_active_buffer(view_mode: ViewMode) {
    let Some((mut editor, tx)) = editor_with_two_page_pdf() else {
      return;
    };
    editor.buffers[0].offset = 3;
    editor.buffers[0].cursor_y = 2;

    let mut panel = BufferState::new(vec![
      "panel-0".to_string(),
      "panel-1".to_string(),
      "panel-2".to_string(),
      "panel-3".to_string(),
    ]);
    panel.offset = 1;
    panel.cursor_y = 1;
    panel.viewport_height = 4;
    panel.is_split_buffer = view_mode == ViewMode::HorizontalSplit;
    editor.buffers.push(panel);
    editor.active_buffer = 1;
    editor.active_pane = 1;
    editor.view_mode = view_mode.clone();
    editor.load_buffer_state(1);
    let active_lines = editor.lines.clone();
    let active_offset = editor.offset;
    let active_cursor_y = editor.cursor_y;
    let active_total_lines = editor.total_lines;

    tx.send(PageLoaded::Page {
      page_index: 0,
      rendered_page: rendered_image_page(&["p1 replacement"]),
      replace_existing: true,
    })
    .expect("replacement page should enqueue");

    assert_eq!(editor.drain_pdf_stream(), 1);
    assert_eq!(editor.active_buffer, 1);
    assert_eq!(editor.view_mode, view_mode);
    assert_eq!(editor.lines, active_lines);
    assert_eq!(editor.offset, active_offset);
    assert_eq!(editor.cursor_y, active_cursor_y);
    assert_eq!(editor.total_lines, active_total_lines);
    assert_eq!(editor.buffers[0].lines[0], "p1 replacement");
    assert_eq!(editor.buffers[0].offset, 1);
    assert_eq!(editor.buffers[0].cursor_y, 2);
  }
}
