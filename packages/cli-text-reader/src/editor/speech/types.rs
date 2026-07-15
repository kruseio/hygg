use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

// Synthetic reading cadence for the fake voice. Tuned to roughly match
// Kokoro's observed ~0.3 s/word from the Phase 0 spike. Only needed when the
// real engine is absent (or in tests).
#[cfg(any(not(feature = "tts"), test))]
pub(crate) const BASE_MS: u64 = 140;
#[cfg(any(not(feature = "tts"), test))]
pub(crate) const PER_CHAR_MS: u64 = 55;
#[cfg(any(not(feature = "tts"), test))]
pub(crate) const SLEEP_STEP_MS: u64 = 25; // cancel-check granularity

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

// Narration lifecycle, shared from the worker thread so the UI can show
// feedback while the (heavy, first-run) engine spins up.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TtsStatus {
  // Downloading the model, loading the ONNX engine, or synthesizing the first
  // chunk — anything before audio starts. Drives the `T[ ]` loading spinner.
  Preparing,
  // Audio is playing and word-boundary events are flowing.
  Speaking,
  // The worker failed before/while speaking; carries a human-readable reason.
  // Only constructed by the Kokoro worker (feature = "tts"); the default
  // build reads it (status line) but never produces it.
  #[allow(dead_code)]
  Failed(String),
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
  // Lifecycle phase for status-line feedback (spinner / error), updated by the
  // worker. Shared so the long first-run model download is visible.
  pub status: Arc<Mutex<TtsStatus>>,
}
