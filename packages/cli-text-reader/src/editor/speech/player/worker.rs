// Background narration worker: spawn the thread, drive the synth-ahead
// playback loop, pace word-boundary emission against the audio clock.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use std::num::NonZero;

use rodio::Player;
use rodio::buffer::SamplesBuffer;

use crate::editor::speech::kokoro::{
  self, KokoroEngine, SAMPLE_RATE, WordAlignment,
};
use crate::editor::speech::{SpeechMsg, SpeechState, TtsStatus};

use super::chunking::{
  AudioClock, build_utterance_chunks, chunk_to_synth_text, is_punct,
  map_words_to_alignments, trailing_pause_secs,
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

  // The output stream must outlive the player; keep it on this thread.
  let stream =
    rodio::DeviceSinkBuilder::open_default_sink().map_err(|e| e.to_string())?;
  let sink = Player::connect_new(stream.mixer());

  let chunks = build_utterance_chunks(words, speed);

  // Word highlights are paced against the audio actually produced by the sink,
  // not a wall clock: `clock` turns the sink's per-chunk playback position into
  // one global timeline, and `chunk_durs` records each queued chunk's length so
  // it can roll finished chunks into that timeline. If playback stalls (e.g. an
  // output-device switch), the position stops advancing and the highlights wait
  // with it, instead of racing ahead.
  let mut clock = AudioClock::new();
  let mut chunk_durs: Vec<f32> = Vec::new();
  let mut elapsed_sec = 0.0f32; // global audio time at the current chunk's start
  let mut prepared = synth_chunk(&mut engine, chunks.first(), voice, speed)?;

  for idx in 0..chunks.len() {
    if cancel.load(Ordering::Relaxed) {
      break;
    }
    let Some((mut audio, aligns)) = prepared.take() else {
      break;
    };

    // Pad the chunk with trailing silence so a sentence/paragraph boundary
    // reads as a real pause (Kokoro renders almost none at a chunk's end, and
    // chunks play back-to-back). A mid-sentence split gets none, staying
    // gapless.
    let pause = chunks[idx].last().map_or(0.0, |last| {
      trailing_pause_secs(last, chunks.get(idx + 1).and_then(|c| c.first()))
    });
    audio.resize(audio.len() + (pause * SAMPLE_RATE as f32) as usize, 0.0);

    let chunk_dur = audio.len() as f32 / SAMPLE_RATE as f32;
    sink.append(SamplesBuffer::new(
      NonZero::new(1).unwrap(),
      NonZero::new(SAMPLE_RATE).unwrap(),
      audio,
    ));
    chunk_durs.push(chunk_dur);

    // Audio for this chunk is now queued: flip the UI from the loading spinner
    // to "speaking" on the first chunk.
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
          if !wait_for_audio(
            &sink,
            &mut clock,
            &chunk_durs,
            elapsed_sec + start_sec,
            cancel,
          ) {
            interrupted = true;
            break;
          }
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
  let text = chunk_to_synth_text(chunk);
  Ok(Some(engine.synthesize(&text, voice, speed)?))
}

// Block until global audio playback reaches `target_sec`, advancing `clock`
// from the sink's reported position each poll. Returns false if cancelled (the
// caller treats that as an interrupt and stops). Polls finely so a chunk's
// reset is never skipped and word timing stays tight.
fn wait_for_audio(
  sink: &Player,
  clock: &mut AudioClock,
  chunk_durs: &[f32],
  target_sec: f32,
  cancel: &AtomicBool,
) -> bool {
  loop {
    if cancel.load(Ordering::Relaxed) {
      return false;
    }
    if clock.observe(sink.get_pos().as_secs_f32(), chunk_durs) >= target_sec {
      return true;
    }
    std::thread::sleep(Duration::from_millis(10));
  }
}
