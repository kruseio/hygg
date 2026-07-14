use super::super::core::Editor;

impl Editor {
  /// Per-iteration housekeeping run at the top of the main loop before any
  /// rendering: poll background PDF work, advance TTS narration, and manage
  /// the "Loaded in X.Xs" indicator.
  pub(crate) fn pre_render_tick(&mut self) {
    // If the PDF is still being opened in the background, see if the
    // opener has finished and install the streaming state when it has.
    if self.pdf_pending.is_some() {
      self.pdf_load_finished = None;
      let _ = self.poll_pending_pdf_stream();
      // Repaint each tick so the elapsed-time counter in the loading
      // splash actually advances while we wait on the opener thread.
      if self.pdf_pending.is_some() {
        self.mark_dirty();
      }
    }
    // Drain any pages the background PDF loader has finished extracting
    // before we render. This keeps the page table in sync and triggers a
    // redraw if anything new arrived.
    if self.pdf_streaming.is_some() {
      let _ = self.drain_pdf_stream();
      if self
        .pdf_streaming
        .as_ref()
        .is_some_and(|state| !state.fully_loaded || state.ocr_loading)
      {
        self.mark_dirty();
      }
    }

    // Advance TTS narration: pull word-boundary events from the worker,
    // move the reading cursor, and repaint so the spoken-word highlight
    // tracks and the viewport auto-scrolls to follow the voice.
    if self.speech.is_some() {
      self.drain_speech();
    }
    // While narration is spinning up (model download / engine load / first
    // synth) no word events arrive, so repaint each tick to animate the
    // `T[ ]` loading spinner.
    if self.is_tts_preparing() {
      self.mark_dirty();
    }

    // Drain background sync notifications (server progress changes). Zero cost
    // when no server is configured.
    if self.sync.is_some() {
      self.poll_sync();
    }
    // Expire the server-progress prompt once the post-scroll grace passes
    // (cheap `Option` check; a no-op until the reader scrolls past a prompt).
    self.tick_server_progress_grace();

    // Accrue active reading time and persist it on a slow cadence while the
    // user is just reading (no cursor movement to trigger a snapshot).
    self.accrue_reading_time();
    self.maybe_flush_reading_time();

    // Manage the "Loaded in X.Xs" indicator: tick through the 500 ms
    // hold so the message appears promptly, then expire after 3 s.
    {
      let load_age = self
        .pdf_load_finished
        .as_ref()
        .map(|(t, _, _)| t.elapsed().as_secs_f32());
      if let Some(age) = load_age {
        if age >= 3.0 {
          self.pdf_load_finished = None;
          self.mark_dirty();
        } else if age < 0.55 {
          self.mark_dirty();
        }
      }
    }

    self.debug_log(&format!(
      "Main loop iteration - buffers: {}, active: {}, mode: {:?}",
      self.buffers.len(),
      self.active_buffer,
      self.view_mode
    ));
    self.debug_log(&format!(
      "  Editor mode: {:?}, command_buffer: '{}'",
      self.editor_state.mode, self.editor_state.command_buffer
    ));
    self.debug_log(&format!(
      "  Active buffer lines: {}, cursor: ({}, {}), offset: {}, needs_redraw: {}, cursor_moved: {}",
      self.lines.len(),
      self.cursor_x,
      self.cursor_y,
      self.offset,
      self.needs_redraw,
      self.cursor_moved
    ));
  }
}
