// Background narration worker: spawn the thread, drive the synth-ahead
// playback loop, pace word-boundary emission against the audio clock.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rodio::buffer::SamplesBuffer;
use rodio::{OutputStream, Sink};

use crate::editor::speech::kokoro::{
  self, KokoroEngine, SAMPLE_RATE, WordAlignment,
};
use crate::editor::speech::{SpeechMsg, SpeechState, TtsStatus};

use super::chunking::{
  build_utterance_chunks, is_punct, map_words_to_alignments,
};
use super::{Chunk, Word};

/// Spawn the Kokoro narration worker. Returns immediately with live state; the
/// first highlight appears once the first chunk has synthesized (and, on first
/// ever use, after the one-time model download).
pub(crate) fn spawn_kokoro_narration(
  words: Vec<Word>,
  voice: String,
  speed: f32,
) -> SpeechState {
  let (tx, rx) = mpsc::channel();
  let cancel = Arc::new(AtomicBool::new(false));
  let status = Arc::new(Mutex::new(TtsStatus::Preparing));
  let cancel_worker = Arc::clone(&cancel);
  let status_worker = Arc::clone(&status);
  let worker = std::thread::Builder::new()
    .name("hygg-tts-kokoro".into())
    .spawn(move || {
      // Surface a real failure (download/load/synth/audio) so the user sees
      // *why* nothing played, instead of a silent dead screen. A user-initiated
      // stop (cancel set) is not an error.
      if let Err(e) =
        run(words, &voice, speed, &tx, &cancel_worker, &status_worker)
        && !cancel_worker.load(Ordering::Relaxed)
        && let Ok(mut s) = status_worker.lock()
      {
        *s = TtsStatus::Failed(e);
      }
      let _ = tx.send(SpeechMsg::Finished);
    })
    .ok();
  SpeechState { rx, cancel, worker, current: None, playing: true, status }
}

fn run(
  words: Vec<Word>,
  voice: &str,
  speed: f32,
  tx: &Sender<SpeechMsg>,
  cancel: &AtomicBool,
  status: &Mutex<TtsStatus>,
) -> Result<(), String> {
  let (model, voices) = kokoro::ensure_models(cancel)?;
  let mut engine = KokoroEngine::load(&model, &voices)?;

  // The output stream must outlive the sink; keep it on this thread.
  let (_stream, handle) =
    OutputStream::try_default().map_err(|e| e.to_string())?;
  let sink = Sink::try_new(&handle).map_err(|e| e.to_string())?;

  let chunks = build_utterance_chunks(words, speed);

  // Anchor the highlight clock to when the first audio is actually queued —
  // NOT to now. Synthesizing the first chunk (and, on first run, downloading
  // the model) takes real time; starting the clock here would make every
  // highlight lead the voice by that amount.
  let mut play_start: Option<Instant> = None;
  let mut elapsed_sec = 0.0f32; // audio time queued so far (gapless)
  let mut prepared = synth_chunk(&mut engine, chunks.first(), voice, speed)?;

  for idx in 0..chunks.len() {
    if cancel.load(Ordering::Relaxed) {
      break;
    }
    let Some((audio, aligns)) = prepared.take() else {
      break;
    };
    let chunk_dur = audio.len() as f32 / SAMPLE_RATE as f32;
    sink.append(SamplesBuffer::new(1, SAMPLE_RATE, audio));

    // Audio for this chunk is now queued: start the clock on the first chunk
    // and flip the UI from the loading spinner to "speaking".
    let base = *play_start.get_or_insert_with(Instant::now);
    if idx == 0
      && let Ok(mut s) = status.lock()
    {
      *s = TtsStatus::Speaking;
    }

    let non_punct: Vec<&WordAlignment> =
      aligns.iter().filter(|a| !is_punct(&a.word)).collect();
    let mapped = map_words_to_alignments(&chunks[idx], &non_punct);

    // Synthesize the NEXT chunk on a helper thread WHILE this chunk's word
    // highlights are emitted, paced by the playback clock. Doing the synthesis
    // inline would block emission for its whole duration; the audio keeps
    // playing meanwhile, so the first words would already be spoken by the time
    // emission resumes and their highlights would fire in one catch-up burst
    // (visible word-skipping at the start of every chunk). Synthesis (faster
    // than realtime) finishes well within this chunk's playback, so the sink
    // still never drains.
    let mut interrupted = false;
    let synth_next: Result<Option<Chunk>, String> =
      std::thread::scope(|scope| {
        let next = scope.spawn(|| {
          synth_chunk(&mut engine, chunks.get(idx + 1), voice, speed)
        });
        for &(span, start_sec) in &mapped {
          if cancel.load(Ordering::Relaxed) {
            interrupted = true;
            break;
          }
          let target = base + Duration::from_secs_f32(elapsed_sec + start_sec);
          sleep_until(target, cancel);
          let msg = SpeechMsg::Word {
            abs_start: span.abs_start,
            abs_end: span.abs_end,
            line: span.line,
          };
          if tx.send(msg).is_err() {
            interrupted = true;
            break;
          }
        }
        next.join().unwrap_or_else(|_| Err("synthesis thread panicked".into()))
      });
    prepared = synth_next?;
    if interrupted {
      return Ok(());
    }
    elapsed_sec += chunk_dur;
  }

  // Let queued audio finish, but stay responsive to a stop request.
  while !cancel.load(Ordering::Relaxed) && !sink.empty() {
    std::thread::sleep(Duration::from_millis(40));
  }
  Ok(())
}

// `Ok(None)` means "no more chunks"; a synthesis failure propagates as `Err`
// so the worker can surface it instead of silently stopping.
fn synth_chunk(
  engine: &mut KokoroEngine,
  chunk: Option<&Vec<Word>>,
  voice: &str,
  speed: f32,
) -> Result<Option<Chunk>, String> {
  let Some(chunk) = chunk else {
    return Ok(None);
  };
  let text =
    chunk.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>().join(" ");
  Ok(Some(engine.synthesize(&text, voice, speed)?))
}

fn sleep_until(deadline: Instant, cancel: &AtomicBool) {
  loop {
    if cancel.load(Ordering::Relaxed) {
      return;
    }
    let now = Instant::now();
    if now >= deadline {
      return;
    }
    std::thread::sleep((deadline - now).min(Duration::from_millis(25)));
  }
}
