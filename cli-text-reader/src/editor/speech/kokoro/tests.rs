use std::path::PathBuf;

use crate::editor::speech::vocab::tokenize;

use super::align::{build_alignments, phonemize, split_words_and_punct};
use super::common::{MAX_TOKENS, SAMPLE_RATE};
use super::engine::KokoroEngine;
use super::files::{model_path, voices_path};

// Input past the model's token limit (which otherwise errors with "invalid
// expand shape") must be split and stitched, not dropped or rejected.
#[test]
#[ignore = "requires the Kokoro model + espeak-ng"]
fn synthesizes_overlong_input_by_splitting() {
  if !model_path().exists() {
    return;
  }
  let mut engine =
    KokoroEngine::load(&model_path(), &voices_path()).expect("load engine");
  let unit = "Git is a distributed version control system that lets \
    developers collaborate on a shared repository by committing snapshots and \
    pushing changes to remote servers. ";
  let text = unit.repeat(3); // ~70 words / >510 tokens
  assert!(
    tokenize(&phonemize(&text)).len() > MAX_TOKENS,
    "test text should exceed the model token limit"
  );

  let (audio, aligns) =
    engine.synthesize(&text, "af_heart", 1.0).expect("must not error");

  assert!(!audio.is_empty(), "split synthesis should still produce audio");
  for pair in aligns.windows(2) {
    assert!(pair[1].start_sec >= pair[0].start_sec, "monotonic timings");
  }
  // Essentially every spoken word should still be aligned (none dropped).
  let words = text.split_whitespace().count();
  assert!(
    aligns.len() >= words - words / 5,
    "expected ~{words} alignments across the split, got {}",
    aligns.len()
  );
}

#[test]
fn split_words_and_punct_separates_trailing_marks() {
  assert_eq!(
    split_words_and_punct("Hello, world!"),
    vec!["Hello", ",", "world", "!"]
  );
}

#[test]
fn build_alignments_scales_to_fast_audio_duration() {
  let word_map = vec![
    ("specific".to_string(), 0, 1),
    ("versions".to_string(), 1, 2),
    ("later".to_string(), 2, 3),
  ];
  let durations = vec![0.0, 40.0, 40.0, 40.0, 0.0];

  let alignments =
    build_alignments(&word_map, &durations, 1, 2.0, SAMPLE_RATE as usize);

  assert_eq!(alignments.len(), 3);
  assert!((alignments[0].start_sec - 0.0).abs() < 0.001);
  assert!((alignments[1].start_sec - 0.333).abs() < 0.01);
  assert!((alignments[2].start_sec - 0.667).abs() < 0.01);
  assert!((alignments[2].end_sec - 1.0).abs() < 0.001);
}

// Real synthesis against the downloaded model. Ignored by default (needs the
// model + espeak). Point HYGG_TTS_MODEL_DIR at a dir with the model+voices,
// or rely on the spike paths. Run:
//   cargo test -p cli-text-reader --features tts --lib \
//     editor::speech::kokoro::tests::synthesizes_with_timings -- --ignored
// --nocapture
#[test]
#[ignore = "requires the Kokoro model + espeak-ng"]
fn synthesizes_with_timings() {
  let (model, voices) = if model_path().exists() {
    (model_path(), voices_path())
  } else {
    (
      PathBuf::from("/tmp/hygg-tts-spike/Kokoros/checkpoints/model.onnx"),
      PathBuf::from("/tmp/hygg-tts-spike/Kokoros/data/voices-v1.0.bin"),
    )
  };
  let mut engine = KokoroEngine::load(&model, &voices).expect("load engine");
  let (audio, words) = engine
    .synthesize("The quick brown fox. Reading aloud now.", "af_sarah", 1.0)
    .expect("synthesize");

  assert!(!audio.is_empty(), "should produce audio");
  assert!(words.len() >= 5, "should align several words, got {}", words.len());
  // Monotonic non-decreasing start times.
  for pair in words.windows(2) {
    assert!(pair[1].start_sec >= pair[0].start_sec);
  }
  for w in &words {
    println!("{:>10}  {:.3}..{:.3}", w.word, w.start_sec, w.end_sec);
  }
}
