// Word-alignment logic: grapheme->phoneme via espeak-ng, per-word token-span
// mapping, and conversion of the model's per-token durations into per-word
// (word, start_sec, end_sec) timings. Ported from the Kokoros project.

use std::sync::Mutex;

use espeak_rs::text_to_phonemes;

use crate::editor::speech::vocab::tokenize;

use super::common::{SAMPLE_RATE, WordMap, WordSpanItem};

// Duration frames are hop=600 @ 24 kHz => 40 frames/sec.
const FRAMES_PER_SEC: f32 = 40.0;

// espeak-ng keeps global state and is not thread-safe; serialize all calls.
static ESPEAK_LOCK: Mutex<()> = Mutex::new(());

/// One narrated word with audio-relative timing.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WordAlignment {
  pub word: String,
  pub start_sec: f32,
  pub end_sec: f32,
}

pub(crate) fn phonemize(text: &str) -> String {
  let _guard = ESPEAK_LOCK.lock().unwrap_or_else(|e| e.into_inner());
  // espeak-rs 0.2 always strips inline language-switch markers (old
  // `remove_lang_switch_flags = true`) and keeps stress (old
  // `remove_stress = false`), so those two flags are gone from the signature.
  text_to_phonemes(text, "en-us", None).unwrap_or_default().join("")
}

// The clause marks we split on and turn into Kokoro pause tokens. Most are in
// the Kokoro vocab, so each maps to a single pause token; the ASCII hyphen `-`
// and en dash `–` are NOT in the vocab, but PDFs routinely use them where the
// book printed an em dash ("a word - another"), so `pause_tokens` maps them to
// the em dash (`—`) so they still pause. Membership is tested per `char`
// because the dashes/ellipsis are multi-byte. A hyphen only ever becomes a mark
// as a *standalone* token: `split_words_and_punct` peels marks off a word's
// ends only, so an interior hyphen ("well-known") stays in the word and never
// pauses.
const PUNCT: &str = ".,!?:;—…-–";

/// A single clause mark (`,` `.` `!` `?` `:` `;` `—` `…` `-` `–`).
pub(crate) fn is_punct_mark(s: &str) -> bool {
  let mut chars = s.chars();
  matches!((chars.next(), chars.next()), (Some(c), None) if PUNCT.contains(c))
}

/// Tokenize a clause mark into its Kokoro pause token(s). The ASCII hyphen and
/// en dash are absent from the model vocab, so map them to the em dash (which
/// is present) — otherwise they tokenize to nothing and produce no pause.
fn pause_tokens(mark: &str) -> Vec<i64> {
  match mark {
    "-" | "–" => tokenize("—"),
    _ => tokenize(mark),
  }
}

/// Build the Kokoro token stream and a per-word token-span map.
///
/// espeak-rs 0.2 strips clause punctuation from its phoneme output (a comma
/// vanishes and the words around it run together — `"two, we"` -> `"tˈuːwiː"`),
/// so feeding the whole phrase through espeak leaves Kokoro with no `,`/`.`
/// tokens and it never pauses. We restore the pauses by splitting the text into
/// punctuation-free runs, phonemizing each run on its own (espeak only
/// coarticulates within a clause anyway, and Kokoro re-derives its prosody from
/// the tokens, so this preserves quality), and emitting each separating mark as
/// its own one-token span. Building the stream and the spans together keeps
/// every word span aligned to the model's per-token `durations`.
pub(crate) fn tokenize_with_alignment(text: &str) -> (Vec<i64>, WordMap) {
  assemble_tokens(&split_words_and_punct(text), |phrase| {
    tokenize(&phonemize(phrase))
  })
}

/// Core of [`tokenize_with_alignment`], with the espeak phonemizer injected so
/// the run/punct/span bookkeeping is unit-testable without espeak. `phon` maps
/// a phrase (a space-joined run of words, or a single word) to its phoneme
/// tokens. Clause marks are tokenized directly — espeak drops them, so we add
/// them back as Kokoro pause tokens.
pub(super) fn assemble_tokens(
  items: &[String],
  mut phon: impl FnMut(&str) -> Vec<i64>,
) -> (Vec<i64>, WordMap) {
  let mut all_tokens: Vec<i64> = Vec::new();
  let mut word_map: WordMap = Vec::with_capacity(items.len());

  let mut i = 0;
  while i < items.len() {
    // A clause mark: one pause token, kept as its own span so the following
    // words still index the right per-token durations.
    if is_punct_mark(&items[i]) {
      let start = all_tokens.len();
      all_tokens.extend(pause_tokens(&items[i]));
      word_map.push((items[i].clone(), start, all_tokens.len()));
      i += 1;
      continue;
    }
    // A maximal run of words between marks: phonemize together for prosody,
    // then split its tokens across the words by per-word phoneme length.
    let run_start = i;
    while i < items.len() && !is_punct_mark(&items[i]) {
      i += 1;
    }
    let run = &items[run_start..i];
    let run_tokens = phon(&run.join(" "));
    let raw: Vec<usize> = run.iter().map(|w| phon(w).len()).collect();
    let counts = apportion(&raw, run_tokens.len());
    let mut cursor = all_tokens.len();
    all_tokens.extend(&run_tokens);
    for (word, cnt) in run.iter().zip(counts) {
      word_map.push((word.clone(), cursor, cursor + cnt));
      cursor += cnt;
    }
  }

  (all_tokens, word_map)
}

/// Largest-remainder apportionment: divide `total` into per-bucket counts
/// weighted by `raw`. espeak coarticulation makes the per-word sum drift from
/// the run's joint total, so the largest fractional remainders absorb the
/// difference. Always sums to exactly `total`, so the word spans tile the run
/// with no gap or overlap. Pure (no espeak), so it is unit-tested directly.
pub(super) fn apportion(raw: &[usize], total: usize) -> Vec<usize> {
  let sum: usize = raw.iter().sum();
  if sum == 0 {
    // Nothing phonemized (e.g. a lone numeral espeak voiced as silence): dump
    // the whole run onto the last word so the spans still cover every token.
    let mut counts = vec![0usize; raw.len()];
    if let Some(last) = counts.last_mut() {
      *last = total;
    }
    return counts;
  }
  let scale = total as f64 / sum as f64;
  let mut counts = vec![0usize; raw.len()];
  let mut frac: Vec<(usize, f64)> = Vec::with_capacity(raw.len());
  let mut assigned = 0usize;
  for (idx, &c) in raw.iter().enumerate() {
    let scaled = c as f64 * scale;
    counts[idx] = scaled.floor() as usize;
    assigned += counts[idx];
    frac.push((idx, scaled - scaled.floor()));
  }
  let mut remaining = total.saturating_sub(assigned);
  frac
    .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
  for (idx, _) in frac {
    if remaining == 0 {
      break;
    }
    counts[idx] += 1;
    remaining -= 1;
  }
  counts
}

pub(crate) fn split_words_and_punct(s: &str) -> Vec<String> {
  let mut out = Vec::new();
  for raw in s.split_whitespace() {
    let chars: Vec<char> = raw.chars().collect();
    let mut start = 0usize;
    let mut end = chars.len();
    while start < end && PUNCT.contains(chars[start]) {
      out.push(chars[start].to_string());
      start += 1;
    }
    while end > start && PUNCT.contains(chars[end - 1]) {
      end -= 1;
    }
    if start < end {
      out.push(chars[start..end].iter().collect());
    }
    for c in chars.iter().take(chars.len()).skip(end) {
      out.push(c.to_string());
    }
  }
  out
}

pub(crate) fn build_alignments(
  word_map: &[WordSpanItem],
  durations: &[f32],
  index_offset: usize,
  speed: f32,
  audio_len: usize,
) -> Vec<WordAlignment> {
  let speed_safe = if speed > 1e-6 { speed } else { 1.0 };
  let punct_pause = |label: &str| -> f32 {
    match label {
      "." | "!" | "?" | "—" | "…" | "-" | "–" => 0.300,
      "," => 0.150,
      ";" | ":" => 0.200,
      _ => 0.0,
    }
  };

  let mut alignments = Vec::new();
  let mut cursor_frames = 0.0f32;
  for (word, start, end) in word_map {
    let is_punct = is_punct_mark(word);
    if is_punct {
      let pause_frames = punct_pause(word) / speed_safe * FRAMES_PER_SEC;
      let start_sec = cursor_frames / FRAMES_PER_SEC;
      let end_sec = (cursor_frames + pause_frames) / FRAMES_PER_SEC;
      alignments.push(WordAlignment { word: word.clone(), start_sec, end_sec });
      cursor_frames += pause_frames;
      continue;
    }
    // Always emit one alignment per spoken word — even a word that phonemized
    // to zero tokens (so it has no duration). Skipping it would desync the
    // player's positional word↔alignment mapping and drop later words.
    let (adj_start, adj_end) = (start + index_offset, end + index_offset);
    let word_frames: f32 = if adj_start < adj_end && adj_end <= durations.len()
    {
      durations[adj_start..adj_end].iter().sum()
    } else {
      0.0
    };
    let start_sec = cursor_frames / FRAMES_PER_SEC;
    let end_sec = (cursor_frames + word_frames) / FRAMES_PER_SEC;
    alignments.push(WordAlignment { word: word.clone(), start_sec, end_sec });
    cursor_frames += word_frames;
  }

  // Linearly scale alignment times to the actual audio length to kill drift.
  // This matters most for `:speed 1.5` / `:speed 2`: the model returns shorter
  // audio, and clamping the correction near 1.0 makes word events lag behind
  // playback until they arrive in visible catch-up bursts.
  let aligned_sec = cursor_frames / FRAMES_PER_SEC;
  let audio_sec = audio_len as f32 / SAMPLE_RATE as f32;
  if aligned_sec > 0.0 {
    let scale = audio_sec / aligned_sec;
    if (scale - 1.0).abs() > 0.005 {
      for a in &mut alignments {
        a.start_sec *= scale;
        a.end_sec *= scale;
      }
    }
  }
  alignments
}
