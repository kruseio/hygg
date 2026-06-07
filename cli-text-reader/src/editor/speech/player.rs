// Kokoro narration worker (feature = "tts").
//
// Runs on a background thread: ensure the model is present, load the engine,
// then chunk the words, synthesize one chunk ahead (synthesis is faster than
// realtime, so the rodio sink stays fed and audio is gapless), play the audio,
// and emit the shared `SpeechMsg::Word` events on the playback clock so the
// existing drain/highlight/auto-scroll path lights up the spoken word.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::time::{Duration, Instant};

use rodio::buffer::SamplesBuffer;
use rodio::{OutputStream, Sink};

use super::kokoro::{self, KokoroEngine, SAMPLE_RATE, WordAlignment};
use super::{SpeechMsg, SpeechState, WordSpan};

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
  let cancel_worker = Arc::clone(&cancel);
  let worker = std::thread::Builder::new()
    .name("hygg-tts-kokoro".into())
    .spawn(move || {
      let _ = run(words, &voice, speed, &tx, &cancel_worker);
      let _ = tx.send(SpeechMsg::Finished);
    })
    .ok();
  SpeechState { rx, cancel, worker, current: None, playing: true }
}

fn run(
  words: Vec<Word>,
  voice: &str,
  speed: f32,
  tx: &Sender<SpeechMsg>,
  cancel: &AtomicBool,
) -> Result<(), String> {
  let (model, voices) = kokoro::ensure_models()?;
  let mut engine = KokoroEngine::load(&model, &voices)?;

  // The output stream must outlive the sink; keep it on this thread.
  let (_stream, handle) =
    OutputStream::try_default().map_err(|e| e.to_string())?;
  let sink = Sink::try_new(&handle).map_err(|e| e.to_string())?;

  let chunks: Vec<Vec<Word>> =
    words.chunks(WORDS_PER_CHUNK).map(<[Word]>::to_vec).collect();

  let play_start = Instant::now();
  let mut elapsed_sec = 0.0f32; // audio time queued so far (gapless)
  let mut prepared = synth_chunk(&mut engine, chunks.first(), voice, speed);

  for idx in 0..chunks.len() {
    if cancel.load(Ordering::Relaxed) {
      break;
    }
    let Some((audio, aligns)) = prepared.take() else {
      break;
    };
    let chunk_dur = audio.len() as f32 / SAMPLE_RATE as f32;
    sink.append(SamplesBuffer::new(1, SAMPLE_RATE, audio));

    // Synthesize the next chunk while this one plays, so the sink never drains.
    prepared = synth_chunk(&mut engine, chunks.get(idx + 1), voice, speed);

    // Emit one Word event per non-punctuation alignment, mapped positionally
    // to this chunk's on-screen word spans.
    let non_punct: Vec<&WordAlignment> =
      aligns.iter().filter(|a| !is_punct(&a.word)).collect();
    for (i, (span, _)) in chunks[idx].iter().enumerate() {
      if cancel.load(Ordering::Relaxed) {
        return Ok(());
      }
      let Some(al) = non_punct.get(i) else {
        break;
      };
      let target = play_start
        + Duration::from_secs_f32(elapsed_sec + al.start_sec.max(0.0));
      sleep_until(target, cancel);
      let msg = SpeechMsg::Word {
        abs_start: span.abs_start,
        abs_end: span.abs_end,
        line: span.line,
      };
      if tx.send(msg).is_err() {
        return Ok(());
      }
    }
    elapsed_sec += chunk_dur;
  }

  // Let queued audio finish, but stay responsive to a stop request.
  while !cancel.load(Ordering::Relaxed) && !sink.empty() {
    std::thread::sleep(Duration::from_millis(40));
  }
  Ok(())
}

fn synth_chunk(
  engine: &mut KokoroEngine,
  chunk: Option<&Vec<Word>>,
  voice: &str,
  speed: f32,
) -> Option<Chunk> {
  let chunk = chunk?;
  let text =
    chunk.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>().join(" ");
  engine.synthesize(&text, voice, speed).ok()
}

fn is_punct(word: &str) -> bool {
  word.len() == 1 && ".,!?:;".contains(word)
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
