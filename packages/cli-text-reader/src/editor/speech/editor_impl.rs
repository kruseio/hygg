use std::io::{Result as IoResult, Write};
use std::sync::atomic::Ordering;

use crossterm::QueueableCommand;
use crossterm::style::{
  Color, ResetColor, SetBackgroundColor, SetForegroundColor,
};
use crossterm::terminal::{Clear, ClearType};

use super::super::core::Editor;
use super::{SpeechMsg, TtsStatus, WordSpan, build_word_spans};

impl Editor {
  // Begin narrating from the current reading line. With the `tts` feature this
  // uses the local Kokoro engine (real audio + word timings); otherwise a
  // silent fake voice that still drives the highlight + auto-scroll.
  pub(crate) fn start_narration(&mut self) {
    self.stop_narration();
    // Master switch (ENABLE_TTS / `--tts off`): narration is inert when off.
    if !self.tts_enabled {
      return;
    }
    let mut all = build_word_spans(&self.lines, &self.line_kinds);
    let current_line = self.offset + self.cursor_y;
    // Narrate from the first word at or after the reading line. If there is no
    // such word — the cursor sits on ASCII art, on a blank/placeholder row, or
    // past the last narratable line (all common on a streaming PDF whose lower
    // pages haven't rendered text yet) — do nothing. Falling back to the start
    // of the document here would yank the reading cursor (and the saved
    // progress) to line 0, so re-opening the document would resume at the top.
    let Some(start_idx) = all.iter().position(|s| s.line >= current_line)
    else {
      return;
    };
    // Drop the already-read head in place rather than cloning the tail with
    // `to_vec`: `all` holds one span per word in the whole document, so a copy
    // here is a second document-sized allocation. `drain` reuses the buffer.
    all.drain(..start_idx);
    let spans: Vec<WordSpan> = all;

    #[cfg(feature = "tts")]
    {
      // Pair each span with its on-screen text so the worker can synthesize.
      let words: Vec<(WordSpan, String)> = spans
        .iter()
        .map(|s| {
          let text = self
            .lines
            .get(s.line)
            .and_then(|l| l.get(s.col_start..s.col_end))
            .unwrap_or_default()
            .to_string();
          (*s, text)
        })
        .collect();
      self.speech = Some(super::player::spawn_kokoro_narration(
        words,
        self.tts_voice.clone(),
        self.tts_speed,
      ));
      self.mark_dirty();
    }

    #[cfg(not(feature = "tts"))]
    self.start_fake_narration(spans);
  }

  // Silent fake voice that still drives the highlight + auto-scroll. Used when
  // the `tts` feature is off, and by the visual demo test.
  #[cfg(any(not(feature = "tts"), test))]
  pub(crate) fn start_fake_narration(&mut self, spans: Vec<WordSpan>) {
    if spans.is_empty() {
      return;
    }
    self.speech =
      Some(super::fake_voice::spawn_fake_narration(spans, self.tts_speed));
    self.mark_dirty();
  }

  // Stop narration and clear the highlight. Detaches the worker (it observes
  // the cancel flag and exits within one sleep step) to avoid blocking the UI.
  pub(crate) fn stop_narration(&mut self) {
    if let Some(state) = self.speech.take() {
      state.cancel.store(true, Ordering::Relaxed);
    }
    self.mark_dirty();
  }

  pub(crate) fn is_narrating(&self) -> bool {
    self.speech.as_ref().is_some_and(|s| s.playing)
  }

  // True while narration is spinning up (model download / engine load / first
  // synth). Drives the `T[ ]` loading spinner and keeps the frame repainting.
  pub(crate) fn is_tts_preparing(&self) -> bool {
    self.speech.as_ref().is_some_and(|s| {
      s.playing
        && s
          .status
          .lock()
          .map(|st| *st == TtsStatus::Preparing)
          .unwrap_or(false)
    })
  }

  // The worker's failure reason, if it errored, for the status line.
  pub(crate) fn tts_error_message(&self) -> Option<String> {
    match &*self.speech.as_ref()?.status.lock().ok()? {
      TtsStatus::Failed(msg) => Some(msg.clone()),
      _ => None,
    }
  }

  // Drain word-boundary events: advance the spoken-word highlight, move the
  // reading cursor (so the existing center_cursor scrolls to follow), repaint.
  pub(crate) fn drain_speech(&mut self) {
    let messages: Vec<SpeechMsg> = match self.speech.as_ref() {
      Some(state) => state.rx.try_iter().collect(),
      None => return,
    };
    if messages.is_empty() {
      return;
    }
    let mut focus_line = None;
    {
      let state = self.speech.as_mut().expect("checked above");
      for message in messages {
        match message {
          SpeechMsg::Word { abs_start, abs_end, line } => {
            state.current = Some((abs_start, abs_end));
            focus_line = Some(line);
          }
          SpeechMsg::Finished => {
            state.current = None;
            state.playing = false;
          }
        }
      }
    }
    if let Some(line) = focus_line {
      self.set_focus_line(line);
    }
    self.mark_dirty();
  }

  // Make `line` the reading line; center_cursor (called each redraw) recenters
  // the viewport around it, which is what produces the smooth auto-scroll.
  fn set_focus_line(&mut self, line: usize) {
    let line = line.min(self.total_lines.saturating_sub(1));
    if line >= self.offset {
      self.cursor_y = line - self.offset;
    } else {
      self.offset = line;
      self.cursor_y = 0;
    }
    self.cursor_moved = true;
  }

  // Does the currently spoken word intersect the given screen row?
  pub(crate) fn spoken_word_on_line(&self, screen_row: usize) -> bool {
    let Some(state) = self.speech.as_ref() else {
      return false;
    };
    let Some((word_start, word_end)) = state.current else {
      return false;
    };
    let line_idx = self.offset + screen_row;
    let Some((line_start, line_end)) = Self::persistent_highlight_line_range(
      line_idx,
      &self.lines,
      &self.line_kinds,
    ) else {
      return false;
    };
    word_start < line_end && word_end > line_start
  }

  // Render `line` with the spoken word styled. Mirrors the persistent-highlight
  // renderer's structure so it composes with the rest of the frame.
  pub(crate) fn highlight_spoken_word_buffered(
    &self,
    buffer: &mut Vec<u8>,
    screen_row: usize,
    line: &str,
    center_offset_string: &str,
  ) -> IoResult<bool> {
    let Some(state) = self.speech.as_ref() else {
      return Ok(false);
    };
    let Some((word_start, word_end)) = state.current else {
      return Ok(false);
    };
    let line_idx = self.offset + screen_row;
    let Some((line_start, _line_end)) = Self::persistent_highlight_line_range(
      line_idx,
      &self.lines,
      &self.line_kinds,
    ) else {
      return Ok(false);
    };

    let start = word_start.saturating_sub(line_start).min(line.len());
    let end = word_end.saturating_sub(line_start).min(line.len());
    if start >= end {
      return Ok(false);
    }

    write!(buffer, "{center_offset_string}")?;
    if start > 0 {
      write!(buffer, "{}", &line[..start])?;
    }
    buffer.queue(SetBackgroundColor(Color::Cyan))?;
    buffer.queue(SetForegroundColor(Color::Black))?;
    write!(buffer, "{}", &line[start..end])?;
    buffer.queue(ResetColor)?;
    if end < line.len() {
      write!(buffer, "{}", &line[end..])?;
    }
    buffer.queue(Clear(ClearType::UntilNewLine))?;
    Ok(true)
  }
}
