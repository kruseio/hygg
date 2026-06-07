// TTS narration — Phase 1 vertical slice.
//
// This module proves the *reading UX* end to end with zero ML/audio
// dependencies: it builds per-word spans from the on-screen lines, runs a
// background "fake voice" that emits word-boundary events on a synthetic
// reading clock, and drives a live "spoken word" highlight plus cursor
// auto-scroll through the existing render loop.
//
// The real Kokoro engine (Phase 2, `kokoro` submodule, feature = "tts") emits
// the same `SpeechMsg::Word` events from actual audio timings; everything
// downstream (drain, highlight, auto-scroll) is shared with the fake voice.

#[cfg(feature = "tts")]
mod kokoro;
#[cfg(feature = "tts")]
mod player;
#[cfg(feature = "tts")]
mod vocab;

use std::io::{Result as IoResult, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use cli_pdf_to_text::PdfLineKind;
use crossterm::QueueableCommand;
use crossterm::style::{
  Color, ResetColor, SetBackgroundColor, SetForegroundColor,
};
use crossterm::terminal::{Clear, ClearType};

use super::core::Editor;

// Synthetic reading cadence for the fake voice. Tuned to roughly match
// Kokoro's observed ~0.3 s/word from the Phase 0 spike. Only needed when the
// real engine is absent (or in tests).
#[cfg(any(not(feature = "tts"), test))]
const BASE_MS: u64 = 140;
#[cfg(any(not(feature = "tts"), test))]
const PER_CHAR_MS: u64 = 55;
#[cfg(any(not(feature = "tts"), test))]
const SLEEP_STEP_MS: u64 = 25; // cancel-check granularity

// What the `:speak` command requested.
#[derive(Debug, PartialEq, Clone, Copy)]
pub(crate) enum SpeakAction {
  Start,
  Stop,
}

// One narratable word, located in BOTH coordinate systems we need:
//   * `abs_start/abs_end` — document byte offsets in the *same* space the
//     persistent-highlight renderer uses (`persistent_highlight_line_range`),
//     so the spoken-word highlight reuses that math unchanged.
//   * `line` + `col_*` — display-line + byte columns, for cursor auto-scroll.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct WordSpan {
  pub abs_start: usize,
  pub abs_end: usize,
  pub line: usize,
  pub col_start: usize,
  pub col_end: usize,
}

// Messages from the narration worker to the main loop.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum SpeechMsg {
  Word { abs_start: usize, abs_end: usize, line: usize },
  Finished,
}

// Live narration state held on the Editor. Std-only, so it compiles in the
// default build with no feature flag — the heavy engine arrives in Phase 2.
pub(crate) struct SpeechState {
  pub rx: Receiver<SpeechMsg>,
  pub cancel: Arc<AtomicBool>,
  #[allow(dead_code)]
  pub worker: Option<JoinHandle<()>>,
  // Currently spoken word as a document byte range, or None between words.
  pub current: Option<(usize, usize)>,
  pub playing: bool,
}

// Byte ranges of whitespace-separated words within a single line. UTF-8 safe:
// indices come from `char_indices`, so every boundary is a char boundary.
pub(crate) fn word_byte_ranges(line: &str) -> Vec<(usize, usize)> {
  let mut ranges = Vec::new();
  let mut start: Option<usize> = None;
  for (idx, ch) in line.char_indices() {
    if ch.is_whitespace() {
      if let Some(s) = start.take() {
        ranges.push((s, idx));
      }
    } else if start.is_none() {
      start = Some(idx);
    }
  }
  if let Some(s) = start {
    ranges.push((s, line.len()));
  }
  ranges
}

// Build the narration word list from the on-screen lines. The absolute-offset
// accumulation MUST match `persistent_highlight_line_range`: non-AnsiArt lines
// contribute `len + 1` (the implicit newline); AnsiArt lines are skipped
// entirely and contribute nothing.
pub(crate) fn build_word_spans(
  lines: &[String],
  line_kinds: &[PdfLineKind],
) -> Vec<WordSpan> {
  let mut spans = Vec::new();
  let mut abs = 0usize;
  for (line_idx, line) in lines.iter().enumerate() {
    if line_kinds.get(line_idx) == Some(&PdfLineKind::AnsiArt) {
      continue; // not in the coordinate space, not narrated
    }
    for (col_start, col_end) in word_byte_ranges(line) {
      spans.push(WordSpan {
        abs_start: abs + col_start,
        abs_end: abs + col_end,
        line: line_idx,
        col_start,
        col_end,
      });
    }
    abs += line.len() + 1;
  }
  spans
}

#[cfg(any(not(feature = "tts"), test))]
fn interruptible_sleep(total_ms: u64, cancel: &AtomicBool) -> bool {
  let mut elapsed = 0;
  while elapsed < total_ms {
    if cancel.load(Ordering::Relaxed) {
      return true;
    }
    let step = SLEEP_STEP_MS.min(total_ms - elapsed);
    std::thread::sleep(Duration::from_millis(step));
    elapsed += step;
  }
  cancel.load(Ordering::Relaxed)
}

// The fake voice: walk the words, emitting each at its synthetic start time.
#[cfg(any(not(feature = "tts"), test))]
fn run_fake_voice(
  spans: Vec<WordSpan>,
  tx: Sender<SpeechMsg>,
  cancel: Arc<AtomicBool>,
  speed: f32,
) {
  let speed = if speed <= 0.0 { 1.0 } else { speed };
  for span in spans {
    if cancel.load(Ordering::Relaxed) {
      break;
    }
    let msg = SpeechMsg::Word {
      abs_start: span.abs_start,
      abs_end: span.abs_end,
      line: span.line,
    };
    if tx.send(msg).is_err() {
      return; // receiver gone (editor dropped the state)
    }
    let chars = (span.col_end.saturating_sub(span.col_start)).max(1) as u64;
    let dur = ((BASE_MS + PER_CHAR_MS * chars) as f32 / speed) as u64;
    if interruptible_sleep(dur, &cancel) {
      break;
    }
  }
  let _ = tx.send(SpeechMsg::Finished);
}

// Spawn the fake-voice worker over a set of word spans.
#[cfg(any(not(feature = "tts"), test))]
fn spawn_fake_narration(spans: Vec<WordSpan>, speed: f32) -> SpeechState {
  let (tx, rx) = mpsc::channel();
  let cancel = Arc::new(AtomicBool::new(false));
  let cancel_worker = Arc::clone(&cancel);
  let worker = std::thread::Builder::new()
    .name("hygg-tts-fake".into())
    .spawn(move || run_fake_voice(spans, tx, cancel_worker, speed))
    .ok();
  SpeechState { rx, cancel, worker, current: None, playing: true }
}

impl Editor {
  // Begin narrating from the current reading line. With the `tts` feature this
  // uses the local Kokoro engine (real audio + word timings); otherwise a
  // silent fake voice that still drives the highlight + auto-scroll.
  pub(crate) fn start_narration(&mut self) {
    self.stop_narration();
    let all = build_word_spans(&self.lines, &self.line_kinds);
    let current_line = self.offset + self.cursor_y;
    let start_idx =
      all.iter().position(|s| s.line >= current_line).unwrap_or(0);
    let spans: Vec<WordSpan> = all[start_idx..].to_vec();
    if spans.is_empty() {
      return;
    }

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
      let (voice, speed) = crate::config::tts_settings();
      self.speech = Some(player::spawn_kokoro_narration(words, voice, speed));
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
    self.speech = Some(spawn_fake_narration(spans, 1.0));
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
  pub(super) fn spoken_word_on_line(&self, screen_row: usize) -> bool {
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
  pub(super) fn highlight_spoken_word_buffered(
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

#[cfg(test)]
mod tests {
  use super::*;
  use std::sync::mpsc;

  fn text_kinds(n: usize) -> Vec<PdfLineKind> {
    vec![PdfLineKind::Text; n]
  }

  #[test]
  fn word_byte_ranges_handles_spaces_and_utf8() {
    assert_eq!(
      word_byte_ranges("the quick fox"),
      vec![(0, 3), (4, 9), (10, 13)]
    );
    // "café" is 5 bytes (é = 2); trailing word offset must be byte-based.
    let line = "café ok";
    assert_eq!(word_byte_ranges(line), vec![(0, 5), (6, 8)]);
    assert_eq!(&line[0..5], "café");
    assert_eq!(&line[6..8], "ok");
  }

  #[test]
  fn build_word_spans_matches_persistent_offset_space() {
    // Two text lines; abs offset of line 1 must be len(line0)+1.
    let lines = vec!["the quick".to_string(), "brown fox".to_string()];
    let spans = build_word_spans(&lines, &text_kinds(2));
    assert_eq!(spans.len(), 4);

    // line 0
    assert_eq!(
      spans[0],
      WordSpan { abs_start: 0, abs_end: 3, line: 0, col_start: 0, col_end: 3 }
    );
    assert_eq!(
      spans[1],
      WordSpan { abs_start: 4, abs_end: 9, line: 0, col_start: 4, col_end: 9 }
    );
    // line 1 starts at len("the quick") + 1 = 10
    assert_eq!(
      spans[2],
      WordSpan {
        abs_start: 10,
        abs_end: 15,
        line: 1,
        col_start: 0,
        col_end: 5
      }
    );
    assert_eq!(
      spans[3],
      WordSpan {
        abs_start: 16,
        abs_end: 19,
        line: 1,
        col_start: 6,
        col_end: 9
      }
    );
  }

  #[test]
  fn build_word_spans_skips_ansi_art_lines() {
    let lines = vec![
      "hello world".to_string(),
      "\x1b[7m▀▀\x1b[0m".to_string(), // ansi art row, skipped
      "again".to_string(),
    ];
    let kinds =
      vec![PdfLineKind::Text, PdfLineKind::AnsiArt, PdfLineKind::Text];
    let spans = build_word_spans(&lines, &kinds);
    // words: hello, world (line 0) + again (line 2); none on the art line.
    assert_eq!(spans.iter().map(|s| s.line).collect::<Vec<_>>(), vec![0, 0, 2]);
    // "again" abs offset = len("hello world")+1 = 12 (art line adds nothing).
    let again = spans.iter().find(|s| s.line == 2).unwrap();
    assert_eq!(again.abs_start, 12);
  }

  // Visual, timing-based end-to-end demo of the real worker thread driving the
  // highlight + auto-scroll. Not run in CI; run explicitly:
  //   cargo test -p cli-text-reader --lib demo_live_narration -- --ignored
  // --nocapture
  #[test]
  #[ignore = "timing-based visual demo"]
  fn demo_live_narration() {
    let lines: Vec<String> =
      (0..14).map(|i| format!("Line {i} reads aloud")).collect();
    let mut editor = Editor::new(lines, 40);
    editor.height = 7; // 6 content rows
    editor.width = 40;
    editor.total_lines = editor.lines.len();
    editor.offset = 0; // start narration from the top so the scroll is visible
    editor.cursor_y = 0;
    let spans = build_word_spans(&editor.lines, &editor.line_kinds);
    editor.start_fake_narration(spans);

    for frame in 0..16 {
      std::thread::sleep(Duration::from_millis(220));
      editor.drain_speech();
      editor.center_cursor(); // the main loop does this every redraw
      let current = editor.speech.as_ref().and_then(|s| s.current);
      println!(
        "\n--- frame {frame}: offset={} cursor_y={} ---",
        editor.offset, editor.cursor_y
      );
      for row in 0..editor.height.saturating_sub(1) {
        let line_idx = editor.offset + row;
        let Some(line) = editor.lines.get(line_idx) else {
          println!("   ~");
          continue;
        };
        let mut shown = line.clone();
        if let (Some((ws, we)), Some((ls, _))) = (
          current,
          Editor::persistent_highlight_line_range(
            line_idx,
            &editor.lines,
            &editor.line_kinds,
          ),
        ) && ws < ls + line.len()
          && we > ls
        {
          let s = ws.saturating_sub(ls).min(line.len());
          let e = we.saturating_sub(ls).min(line.len());
          if s < e {
            shown = format!("{}[{}]{}", &line[..s], &line[s..e], &line[e..]);
          }
        }
        let cursor =
          if line_idx == editor.offset + editor.cursor_y { ">" } else { " " };
        println!("{cursor}  {shown}");
      }
      if !editor.is_narrating() {
        break;
      }
    }
    editor.stop_narration();
  }

  #[test]
  fn renders_highlight_escape_around_spoken_word() {
    let mut editor = Editor::new(vec!["alpha beta gamma".to_string()], 80);
    editor.height = 3;
    editor.width = 80;
    editor.show_highlighter = true;
    editor.speech = Some(SpeechState {
      rx: mpsc::channel().1,
      cancel: Arc::new(AtomicBool::new(false)),
      worker: None,
      current: Some((6, 10)), // "beta"
      playing: true,
    });

    let mut buffer = Vec::new();
    editor.draw_content_buffered(&mut buffer, 80, "").unwrap();
    let out = String::from_utf8_lossy(&buffer);

    // The exact escape crossterm emits for our highlight background.
    let mut want = Vec::new();
    want.queue(SetBackgroundColor(Color::Cyan)).unwrap();
    let want = String::from_utf8(want).unwrap();

    assert!(out.contains(&want), "expected the cyan background escape");
    assert!(out.contains("beta"), "spoken word should be rendered");
    assert!(out.contains("alpha "), "leading text should be rendered");
  }

  #[test]
  fn drain_speech_advances_highlight_and_scrolls() {
    let mut editor = Editor::new(
      vec![
        "line zero".to_string(),
        "line one".to_string(),
        "target two".to_string(),
      ],
      80,
    );
    editor.height = 24;
    editor.total_lines = 3;

    let (tx, rx) = mpsc::channel();
    editor.speech = Some(SpeechState {
      rx,
      cancel: Arc::new(AtomicBool::new(false)),
      worker: None,
      current: None,
      playing: true,
    });

    // "target" on line 2: abs offset = len("line zero")+1+len("line one")+1.
    let abs = "line zero".len() + 1 + "line one".len() + 1;
    tx.send(SpeechMsg::Word { abs_start: abs, abs_end: abs + 6, line: 2 })
      .unwrap();
    editor.drain_speech();

    let state = editor.speech.as_ref().unwrap();
    assert_eq!(state.current, Some((abs, abs + 6)));
    assert!(state.playing);
    // reading line followed the word
    assert_eq!(editor.offset + editor.cursor_y, 2);
    // the spoken word is on screen row (2 - offset) and highlights
    assert!(editor.spoken_word_on_line(2 - editor.offset));

    // Finished clears the highlight and marks not playing.
    tx.send(SpeechMsg::Finished).unwrap();
    editor.drain_speech();
    let state = editor.speech.as_ref().unwrap();
    assert_eq!(state.current, None);
    assert!(!state.playing);
  }
}
