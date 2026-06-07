// Kokoro narration worker (feature = "tts").
//
// Runs on a background thread: ensure the model is present, load the engine,
// then chunk the words, synthesize one chunk ahead (synthesis is faster than
// realtime, so the rodio sink stays fed and audio is gapless), play the audio,
// and emit the shared `SpeechMsg::Word` events on the playback clock so the
// existing drain/highlight/auto-scroll path lights up the spoken word.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rodio::buffer::SamplesBuffer;
use rodio::{OutputStream, Sink};

use super::kokoro::{self, KokoroEngine, SAMPLE_RATE, WordAlignment};
use super::{SpeechMsg, SpeechState, TtsStatus, WordSpan};

// Conservative word budget per synthesis call — well under the model's token
// limit even for long words (~50 words ≈ a few hundred tokens).
const WORDS_PER_CHUNK: usize = 50;

type Word = (WordSpan, String);
type Chunk = (Vec<f32>, Vec<WordAlignment>);

/// Spawn the Kokoro narration worker. Returns immediately with live state; the
/// first highlight appears once the first chunk has synthesized (and, on first
/// ever use, after the one-time model download).
pub(super) fn spawn_kokoro_narration(
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

  let chunks: Vec<Vec<Word>> =
    words.chunks(WORDS_PER_CHUNK).map(<[Word]>::to_vec).collect();

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

// A single punctuation token in the *alignment* stream (filtered out so it
// never claims a highlight slot).
fn is_punct(word: &str) -> bool {
  word.len() == 1 && ".,!?:;".contains(word)
}

// An *on-screen* word that is entirely punctuation (".", "...", "?!", …). Such
// words are not spoken and produce no alignment, so the player skips them to
// keep its positional word↔alignment mapping in sync.
fn is_all_punct(word: &str) -> bool {
  !word.is_empty() && word.chars().all(|c| ".,!?:;".contains(c))
}

// Pair spoken on-screen words with their audio alignments, in order. On-screen
// words that are pure punctuation are not spoken and have no alignment, so they
// are skipped here — keeping the positional mapping 1:1 with `non_punct` so no
// later word is shifted onto the wrong alignment (or dropped off the end).
// Returns each kept word's span and its clamped start time (seconds, relative
// to the chunk's audio start).
fn map_words_to_alignments<'a>(
  spans: &'a [Word],
  non_punct: &[&WordAlignment],
) -> Vec<(&'a WordSpan, f32)> {
  let mut out = Vec::with_capacity(spans.len());
  let mut ai = 0usize;
  for (span, text) in spans {
    if is_all_punct(text) {
      continue;
    }
    let Some(al) = non_punct.get(ai) else {
      break;
    };
    ai += 1;
    out.push((span, al.start_sec.max(0.0)));
  }
  out
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

#[cfg(test)]
mod tests {
  use super::*;

  fn span_at(abs: usize) -> WordSpan {
    WordSpan {
      abs_start: abs,
      abs_end: abs + 1,
      line: 0,
      col_start: 0,
      col_end: 1,
    }
  }

  // Regression: a standalone-punctuation on-screen word (here ",") must not
  // consume an alignment slot, or every following word's highlight shifts by
  // one and the last word gets dropped. "world" must still map to its own
  // alignment (0.5s), not the comma's missing one.
  #[test]
  fn maps_words_skipping_standalone_punctuation() {
    let spans: Vec<Word> = vec![
      (span_at(0), "Hello".to_string()),
      (span_at(1), ",".to_string()),
      (span_at(2), "world".to_string()),
    ];
    let aligns = vec![
      WordAlignment { word: "Hello".into(), start_sec: 0.0, end_sec: 0.5 },
      WordAlignment { word: "world".into(), start_sec: 0.5, end_sec: 1.0 },
    ];
    let non_punct: Vec<&WordAlignment> = aligns.iter().collect();

    let mapped = map_words_to_alignments(&spans, &non_punct);

    assert_eq!(mapped.len(), 2, "comma should be skipped, both words emitted");
    assert_eq!((mapped[0].0.abs_start, mapped[0].1), (0, 0.0)); // "Hello"
    assert_eq!((mapped[1].0.abs_start, mapped[1].1), (2, 0.5)); // "world"
  }

  // Negative start times (alignment scaling can underflow slightly) clamp to 0.
  #[test]
  fn maps_words_clamps_negative_start() {
    let spans: Vec<Word> = vec![(span_at(0), "hi".to_string())];
    let aligns =
      vec![WordAlignment { word: "hi".into(), start_sec: -0.2, end_sec: 0.3 }];
    let non_punct: Vec<&WordAlignment> = aligns.iter().collect();

    let mapped = map_words_to_alignments(&spans, &non_punct);

    assert_eq!(mapped[0].1, 0.0);
  }

  // Full pipeline: load engine, play audio, emit word events. Plays sound and
  // needs the model + espeak + an audio device, so it is ignored by default:
  //   cargo test -p cli-text-reader --features tts --lib \
  //     editor::speech::player::tests::plays_and_emits_words -- --ignored
  // --nocapture
  #[test]
  #[ignore = "plays audio; requires model + espeak + audio device"]
  fn plays_and_emits_words() {
    let words: Vec<Word> = ["Reading", "aloud", "now"]
      .iter()
      .enumerate()
      .map(|(i, t)| {
        let span = WordSpan {
          abs_start: i,
          abs_end: i + 1,
          line: 0,
          col_start: 0,
          col_end: 1,
        };
        (span, t.to_string())
      })
      .collect();

    let state = spawn_kokoro_narration(words, "af_sarah".to_string(), 1.0);
    let mut word_events = 0;
    // Loop ends on Finished / Err (non-Word), counting Word events.
    while let Ok(SpeechMsg::Word { .. }) =
      state.rx.recv_timeout(Duration::from_secs(30))
    {
      word_events += 1;
    }
    assert!(word_events >= 3, "expected >=3 word events, got {word_events}");
  }
}
