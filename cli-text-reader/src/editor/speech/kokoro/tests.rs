use std::path::PathBuf;

use crate::editor::speech::vocab::tokenize;

use super::align::{
  apportion, assemble_tokens, build_alignments, is_punct_mark, phonemize,
  split_words_and_punct, tokenize_with_alignment,
};
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

// ---- Punctuation/alignment regression tests (no espeak, run in CI) ----
//
// These guard the espeak-0.2 regression where clause punctuation was dropped
// from the phoneme stream, so Kokoro lost its `,`/`.` pause tokens and ran
// words together (e.g. "two, we" -> "tˈuːwiː"). `assemble_tokens` takes an
// injected phonemizer, so the run/punct/span logic is checked without espeak.

// One token per alphanumeric char, so a run's joint count equals the sum of its
// words' counts (exact tiling) — lets us assert spans precisely.
fn fake_phon(s: &str) -> Vec<i64> {
  s.chars().filter(|c| c.is_alphanumeric()).map(|_| 99i64).collect()
}

#[test]
fn is_punct_mark_only_matches_single_clause_marks() {
  for m in [",", ".", "!", "?", ":", ";"] {
    assert!(is_punct_mark(m), "{m:?} should be a clause mark");
  }
  for s in ["a", "", ",,", "2", " "] {
    assert!(!is_punct_mark(s), "{s:?} should not be a clause mark");
  }
}

#[test]
fn apportion_always_sums_to_total() {
  for (raw, total) in [
    (vec![2usize, 7, 1], 10usize),
    (vec![1, 1, 1], 10),
    (vec![3, 1], 8),
    (vec![5], 5),
  ] {
    let got = apportion(&raw, total);
    assert_eq!(got.len(), raw.len());
    assert_eq!(got.iter().sum::<usize>(), total, "raw={raw:?} total={total}");
  }
}

#[test]
fn apportion_keeps_exact_counts_when_already_summing() {
  assert_eq!(apportion(&[2, 7, 1], 10), vec![2, 7, 1]);
}

#[test]
fn apportion_all_empty_dumps_to_last() {
  // A run that phonemized to nothing still has every token covered.
  assert_eq!(apportion(&[0, 0, 0], 4), vec![0, 0, 4]);
}

#[test]
fn assemble_inserts_pauses_and_keeps_words_separated() {
  let items = split_words_and_punct("In Chapter 2, we kitchen.");
  let (tokens, wmap) = assemble_tokens(&items, fake_phon);

  // Comma (id 3) and period (id 4) appear as their own pause tokens.
  assert!(tokens.contains(&3), "comma pause token missing: {tokens:?}");
  assert!(tokens.contains(&4), "period pause token missing: {tokens:?}");

  // Spans tile the stream exactly — no gap, no overlap — so every word indexes
  // the correct per-token durations.
  let mut cursor = 0usize;
  for (_, start, end) in &wmap {
    assert_eq!(*start, cursor, "span gap/overlap in {wmap:?}");
    cursor = *end;
  }
  assert_eq!(cursor, tokens.len(), "spans must cover every token");

  // The regression: the comma sits strictly between "2" and "we".
  let pos = |w: &str| wmap.iter().position(|(t, _, _)| t == w).unwrap();
  assert!(pos("2") < pos(",") && pos(",") < pos("we"), "{wmap:?}");
}

#[test]
fn assemble_unpunctuated_text_has_no_pause_tokens() {
  let items = split_words_and_punct("just plain words");
  let (tokens, _) = assemble_tokens(&items, fake_phon);
  assert!(!tokens.contains(&3) && !tokens.contains(&4), "{tokens:?}");
}

// Ground-truth guard that runs the *real* espeak phonemizer end to end, so a
// future espeak/dep bump that again drops clause punctuation fails CI. Skips
// (stays green) when espeak-ng-data can't be located.
#[test]
fn clause_marks_become_pause_tokens_with_real_espeak() {
  if !ensure_espeak_data() || phonemize("test").is_empty() {
    return; // espeak-ng-data unavailable
  }
  let (tokens, wmap) = tokenize_with_alignment("In Chapter 2, we were here.");
  assert!(tokens.contains(&3), "comma pause token missing: {tokens:?}");
  assert!(tokens.contains(&4), "period pause token missing: {tokens:?}");
  assert!(tokens.len() > 4, "expected real word tokens: {tokens:?}");
  let pos = |w: &str| wmap.iter().position(|(t, _, _)| t == w).unwrap();
  assert!(pos("2") < pos(","), "comma must follow '2', not merge into 'we'");
}

// Point espeak at the espeak-ng-data the build vendored under an ancestor's
// target/, so the real-espeak test can run in CI without manual env setup.
fn ensure_espeak_data() -> bool {
  if std::env::var_os("PIPER_ESPEAKNG_DATA_DIRECTORY").is_some() {
    return true;
  }
  let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
  loop {
    if let Some(share) = find_espeak_share(&dir.join("target")) {
      // SAFETY: test-only. This is the sole writer of the variable and the only
      // espeak caller in the suite (the other espeak/model tests are
      // #[ignore]), so it runs before any espeak read with no concurrent
      // access.
      unsafe {
        std::env::set_var("PIPER_ESPEAKNG_DATA_DIRECTORY", &share);
      }
      return true;
    }
    if !dir.pop() {
      return false;
    }
  }
}

fn find_espeak_share(target: &std::path::Path) -> Option<PathBuf> {
  for profile in ["debug", "release"] {
    let build = target.join(profile).join("build");
    let Ok(entries) = std::fs::read_dir(&build) else {
      continue;
    };
    for e in entries.flatten() {
      let share = e.path().join("out").join("share");
      if share.join("espeak-ng-data").join("phontab").is_file() {
        return Some(share);
      }
    }
  }
  None
}
