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
  text_to_phonemes(text, "en-us", None, true, false)
    .unwrap_or_default()
    .join("")
}

/// Tokenize the full phrase (best prosody) and build a per-word token-span map
/// by phonemizing each word/punctuation item on its own. Ported from Kokoros.
pub(crate) fn tokenize_with_alignment(text: &str) -> (Vec<i64>, WordMap) {
  let all_tokens = tokenize(&phonemize(text));

  let items = split_words_and_punct(text);
  let mut counts: Vec<usize> = Vec::with_capacity(items.len());
  let mut is_punct: Vec<bool> = Vec::with_capacity(items.len());
  for item in &items {
    if item.len() == 1 && ".,!?:;".contains(item.as_str()) {
      counts.push(0);
      is_punct.push(true);
    } else {
      counts.push(tokenize(&phonemize(item)).len());
      is_punct.push(false);
    }
  }

  // Rescale per-item counts so they sum to the full token length (espeak
  // coarticulation makes per-word sums drift from the full-phrase count).
  let target = all_tokens.len();
  let sum: usize = counts.iter().sum();
  if sum != target && sum > 0 {
    let scale = target as f64 / sum as f64;
    let mut frac: Vec<(usize, f64)> = Vec::with_capacity(counts.len());
    let mut new_sum = 0usize;
    for (i, &c) in counts.clone().iter().enumerate() {
      let scaled = c as f64 * scale;
      counts[i] = scaled.floor() as usize;
      new_sum += counts[i];
      frac.push((i, scaled - scaled.floor()));
    }
    let mut remaining = target.saturating_sub(new_sum);
    frac.sort_by(|a, b| {
      b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
    });
    for (i, _) in frac {
      if remaining == 0 {
        break;
      }
      counts[i] += 1;
      remaining -= 1;
    }
  }

  let mut word_map = Vec::with_capacity(items.len());
  let mut cursor = 0usize;
  for (idx, item) in items.iter().enumerate() {
    let cnt = counts.get(idx).copied().unwrap_or(0);
    if is_punct[idx] {
      word_map.push((item.clone(), cursor, cursor));
    } else {
      let end = cursor + cnt;
      word_map.push((item.clone(), cursor, end));
      cursor = end;
    }
  }
  // Cover any rounding shortfall by extending the last real word.
  if cursor < target
    && let Some(last) = (0..word_map.len()).rev().find(|&i| !is_punct[i])
  {
    let (w, s, _) = &word_map[last];
    word_map[last] = (w.clone(), *s, target);
  }

  (all_tokens, word_map)
}

pub(crate) fn split_words_and_punct(s: &str) -> Vec<String> {
  let mut out = Vec::new();
  for raw in s.split_whitespace() {
    let chars: Vec<char> = raw.chars().collect();
    let mut start = 0usize;
    let mut end = chars.len();
    while start < end && ".,!?:;".contains(chars[start]) {
      out.push(chars[start].to_string());
      start += 1;
    }
    while end > start && ".,!?:;".contains(chars[end - 1]) {
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
      "." | "!" | "?" => 0.300,
      "," => 0.150,
      ";" | ":" => 0.200,
      _ => 0.0,
    }
  };

  let mut alignments = Vec::new();
  let mut cursor_frames = 0.0f32;
  for (word, start, end) in word_map {
    let is_punct = word.len() == 1 && ".,!?:;".contains(word.as_str());
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
