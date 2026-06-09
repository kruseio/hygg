use std::time::Duration;

use crate::editor::speech::kokoro::WordAlignment;
use crate::editor::speech::{SpeechMsg, WordSpan};

use super::Word;
use super::chunking::{build_utterance_chunks, map_words_to_alignments};
use super::worker::spawn_kokoro_narration;

fn span_at(abs: usize) -> WordSpan {
  WordSpan {
    abs_start: abs,
    abs_end: abs + 1,
    line: 0,
    col_start: 0,
    col_end: 1,
  }
}

fn word(text: &str) -> Word {
  (span_at(0), text.to_string())
}

// Breaks right after a sentence end once past the minimum; the remainder
// becomes its own (short) trailing chunk. No words are lost.
#[test]
fn utterance_chunks_break_on_sentence_past_minimum() {
  let mut words: Vec<Word> = (0..8).map(|i| word(&format!("w{i}"))).collect();
  words.push(word("end.")); // 9th word ends a sentence (>= MIN_CHUNK_WORDS)
  words.extend((0..3).map(|i| word(&format!("x{i}"))));

  let chunks = build_utterance_chunks(words, 1.0);

  assert_eq!(chunks.iter().map(Vec::len).collect::<Vec<_>>(), vec![9, 3]);
}

// A long run with no sentence punctuation is hard-capped at MAX_CHUNK_WORDS.
#[test]
fn utterance_chunks_cap_runs_without_punctuation() {
  let words: Vec<Word> = (0..40).map(|i| word(&format!("w{i}"))).collect();

  let chunks = build_utterance_chunks(words, 1.0);

  assert_eq!(chunks.iter().map(Vec::len).collect::<Vec<_>>(), vec![36, 4]);
  assert_eq!(chunks.iter().map(Vec::len).sum::<usize>(), 40);
}

// Faster playback leaves less real time for synth-ahead, so chunks grow with
// speed. This keeps short sentences from becoming gap-prone boundaries at 2x.
#[test]
fn utterance_chunks_merge_short_sentences_at_fast_speed() {
  let first = "What is version control and why should you care?";
  let second = "Version control is a system that records changes to a file or set of files over time so that you can recall specific versions later.";
  let words: Vec<Word> =
    format!("{first} {second}").split_whitespace().map(word).collect();

  let normal_chunks = build_utterance_chunks(words.clone(), 1.0);
  let fast_chunks = build_utterance_chunks(words, 2.0);

  assert_eq!(
    normal_chunks.iter().map(Vec::len).collect::<Vec<_>>(),
    vec![9, 25]
  );
  assert_eq!(fast_chunks.iter().map(Vec::len).collect::<Vec<_>>(), vec![34]);
}

// Regression: a normal-length book sentence should stay in one utterance.
// Splitting the Pro Git intro sentence at 24 words made Kokoro pause around
// "GitHub is and how to" even though there was no sentence break there.
#[test]
fn utterance_chunks_keep_normal_book_sentence_together() {
  let sentence = "Instead of an example of Git hosting, I have decided to turn that part of the book into more deeply describing what GitHub is and how to effectively use it.";
  let words: Vec<Word> = sentence.split_whitespace().map(word).collect();

  let chunks = build_utterance_chunks(words, 1.0);

  assert_eq!(chunks.iter().map(Vec::len).collect::<Vec<_>>(), vec![30]);
}

// Short consecutive sentences are merged until the minimum, so we never emit
// a one-word utterance (which could underrun the audio sink).
#[test]
fn utterance_chunks_merge_short_sentences() {
  let words: Vec<Word> =
    ["Yes.", "No.", "Maybe.", "I.", "do.", "not.", "know.", "yet.", "more"]
      .iter()
      .map(|t| word(t))
      .collect();

  let chunks = build_utterance_chunks(words, 1.0);

  // First break only once 8 words have accumulated (at "yet."), not at "Yes."
  assert_eq!(chunks[0].len(), 8);
  assert_eq!(chunks.iter().map(Vec::len).sum::<usize>(), 9);
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
  let aligns = [
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
    [WordAlignment { word: "hi".into(), start_sec: -0.2, end_sec: 0.3 }];
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
