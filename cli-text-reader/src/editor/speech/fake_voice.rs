use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::{
  BASE_MS, PER_CHAR_MS, SLEEP_STEP_MS, SpeechMsg, SpeechState, TtsStatus,
  WordSpan,
};

pub(crate) fn interruptible_sleep(total_ms: u64, cancel: &AtomicBool) -> bool {
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
pub(crate) fn run_fake_voice(
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
pub(crate) fn spawn_fake_narration(
  spans: Vec<WordSpan>,
  speed: f32,
) -> SpeechState {
  let (tx, rx) = mpsc::channel();
  let cancel = Arc::new(AtomicBool::new(false));
  let cancel_worker = Arc::clone(&cancel);
  let worker = std::thread::Builder::new()
    .name("hygg-tts-fake".into())
    .spawn(move || run_fake_voice(spans, tx, cancel_worker, speed))
    .ok();
  // The fake voice has nothing to prepare, so it is "speaking" immediately.
  SpeechState {
    rx,
    cancel,
    worker,
    current: None,
    playing: true,
    status: Arc::new(Mutex::new(TtsStatus::Speaking)),
  }
}
