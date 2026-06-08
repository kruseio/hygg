// Local Kokoro-82M TTS engine (feature = "tts").
//
// A lean port of the inference + word-alignment logic from the Kokoros project
// (Apache-2.0): grapheme->phoneme via espeak-ng (`espeak-rs`), tokenize against
// the Kokoro vocab, run the *timestamped* ONNX model via `ort`, and convert the
// per-token duration output into per-word (word, start_sec, end_sec) timings.
//
// Deliberately excludes Kokoros' opus/mp3/ogg encoders, async runtime, and
// HTTP server: hygg only needs raw f32 PCM @ 24 kHz plus the word timings.
// The model is fetched on first use (it is far too large to bundle/publish).

use std::borrow::Cow;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use espeak_rs::text_to_phonemes;
use ndarray::Array3;
use ndarray_npy::NpzReader;
use ort::session::builder::SessionBuilder;
use ort::session::{Session, SessionInputValue, SessionInputs};
use ort::value::{Tensor, Value};

use super::vocab::tokenize;

// Timestamped Kokoro v1.0 ONNX schema.
const TOKENS_KEY: &str = "input_ids";
const STYLE_KEY: &str = "style";
const SPEED_KEY: &str = "speed";
const AUDIO_KEY: &str = "waveform";
const DURATIONS_KEY: &str = "durations";

pub(super) const SAMPLE_RATE: u32 = 24_000;
// Duration frames are hop=600 @ 24 kHz => 40 frames/sec.
const FRAMES_PER_SEC: f32 = 40.0;
const STYLE_DIM: usize = 256;
const MAX_STYLE_ROWS: usize = 510;
// The model rejects inputs past ~510 phoneme tokens (its style table has 511
// rows and the ONNX graph errors with "invalid expand shape" beyond that), and
// gets less accurate as it approaches the limit. Synthesis splits text whose
// phoneme stream exceeds this, leaving comfortable margin.
const MAX_TOKENS: usize = 480;

// espeak-ng keeps global state and is not thread-safe; serialize all calls.
static ESPEAK_LOCK: Mutex<()> = Mutex::new(());

const MODEL_URL: &str = "https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX-timestamped/resolve/main/onnx/model.onnx";
const VOICES_URL: &str = "https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.0/voices-v1.0.bin";

/// One narrated word with audio-relative timing.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct WordAlignment {
  pub word: String,
  pub start_sec: f32,
  pub end_sec: f32,
}

// Per-voice style table: 511 rows of [1][256], indexed by token count.
type VoiceStyles = HashMap<String, Vec<[[f32; STYLE_DIM]; 1]>>;
// (word-or-punct text, token-span start, token-span end) over the token stream.
type WordSpanItem = (String, usize, usize);
type WordMap = Vec<WordSpanItem>;
// One named ONNX session input value.
type SessionInput = (Cow<'static, str>, SessionInputValue<'static>);

// --- model/voice file management -------------------------------------------

/// Directory the model + voices live in. Override with `HYGG_TTS_MODEL_DIR`
/// (for offline / air-gapped use); otherwise the platform cache dir.
pub(super) fn model_dir() -> PathBuf {
  if let Some(dir) = std::env::var_os("HYGG_TTS_MODEL_DIR") {
    return PathBuf::from(dir);
  }
  dirs::cache_dir()
    .unwrap_or_else(|| PathBuf::from("."))
    .join("hygg")
    .join("tts")
}

pub(super) fn model_path() -> PathBuf {
  model_dir().join("kokoro-v1.0-timestamped.onnx")
}

pub(super) fn voices_path() -> PathBuf {
  model_dir().join("voices-v1.0.bin")
}

fn download_to(
  url: &str,
  dest: &Path,
  cancel: &AtomicBool,
) -> Result<(), String> {
  if let Some(parent) = dest.parent() {
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
  }
  let tmp = dest.with_extension("part");
  let body = ureq::get(url)
    .call()
    .map_err(|e| format!("download {url} failed: {e}"))?
    .into_body();
  let mut reader = body.into_reader();
  let mut file = File::create(&tmp).map_err(|e| e.to_string())?;

  // Copy in chunks (rather than io::copy) so a stop request aborts this
  // multi-hundred-MB download promptly instead of running to completion on a
  // detached thread after the user has already pressed a key.
  let mut buf = vec![0u8; 64 * 1024];
  loop {
    if cancel.load(Ordering::Relaxed) {
      drop(file);
      let _ = std::fs::remove_file(&tmp);
      return Err("cancelled".to_string());
    }
    let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
    if n == 0 {
      break;
    }
    file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
  }
  file.flush().map_err(|e| e.to_string())?;
  drop(file);
  std::fs::rename(&tmp, dest).map_err(|e| e.to_string())?;
  Ok(())
}

/// Ensure the model + voices exist locally, downloading on first use. Returns
/// the resolved paths. Network is only touched when a file is missing.
pub(super) fn ensure_models(
  cancel: &AtomicBool,
) -> Result<(PathBuf, PathBuf), String> {
  let (model, voices) = (model_path(), voices_path());
  if !model.exists() {
    download_to(MODEL_URL, &model, cancel)?;
  }
  if !voices.exists() {
    download_to(VOICES_URL, &voices, cancel)?;
  }
  Ok((model, voices))
}

// --- engine -----------------------------------------------------------------

pub(super) struct KokoroEngine {
  session: Session,
  styles: VoiceStyles,
}

impl KokoroEngine {
  /// Load the ONNX model and voice styles from disk.
  pub(super) fn load(
    model_path: &Path,
    voices_path: &Path,
  ) -> Result<Self, String> {
    let session = SessionBuilder::new()
      .map_err(|e| format!("ort session builder: {e}"))?
      .commit_from_file(model_path)
      .map_err(|e| format!("load model {}: {e}", model_path.display()))?;

    if session.outputs().len() <= 1 {
      return Err(
        "model has no durations output — not the timestamped Kokoro model"
          .to_string(),
      );
    }

    let styles = load_voices(voices_path)?;
    Ok(Self { session, styles })
  }

  /// Synthesize `text` with `voice` (e.g. "af_sarah" or
  /// "af_sarah.4+af_nicole.6") at `speed`. Returns 24 kHz mono f32 PCM and
  /// per-word timings.
  pub(super) fn synthesize(
    &mut self,
    text: &str,
    voice: &str,
    speed: f32,
  ) -> Result<(Vec<f32>, Vec<WordAlignment>), String> {
    let (tokens, word_map) = tokenize_with_alignment(text);
    if tokens.is_empty() {
      return Ok((Vec::new(), Vec::new()));
    }

    // Too long for the model: split on a word boundary and stitch the halves so
    // no words are dropped (the chunker keeps most inputs well under this).
    if tokens.len() > MAX_TOKENS {
      let words: Vec<&str> = text.split_whitespace().collect();
      if words.len() > 1 {
        let mid = words.len() / 2;
        let (mut audio, mut alignments) =
          self.synthesize(&words[..mid].join(" "), voice, speed)?;
        let (tail_audio, tail_aligns) =
          self.synthesize(&words[mid..].join(" "), voice, speed)?;
        let shift = audio.len() as f32 / SAMPLE_RATE as f32;
        audio.extend(tail_audio);
        alignments.extend(tail_aligns.into_iter().map(|mut a| {
          a.start_sec += shift;
          a.end_sec += shift;
          a
        }));
        return Ok((audio, alignments));
      }
    }

    let styles = self.mix_styles(voice, tokens.len())?;

    // Pad with BOS/EOS (id 0); durations line up with this padded stream.
    let mut padded = vec![0i64];
    padded.extend(tokens.iter().copied());
    padded.push(0);
    let index_offset = 1usize; // skip the leading pad

    let (audio, durations) = self.infer(vec![padded], styles, speed)?;
    let alignments =
      build_alignments(&word_map, &durations, index_offset, speed, audio.len());
    Ok((audio, alignments))
  }

  fn mix_styles(
    &self,
    style_name: &str,
    tokens_len: usize,
  ) -> Result<Vec<Vec<f32>>, String> {
    let row = tokens_len.min(MAX_STYLE_ROWS);
    if !style_name.contains('+') {
      let style = self
        .styles
        .get(style_name)
        .ok_or_else(|| format!("unknown voice: {style_name}"))?;
      return Ok(vec![style[row][0].to_vec()]);
    }

    // Blend: "name.weight+name.weight", weights are tenths.
    let mut blended = vec![0.0f32; STYLE_DIM];
    for part in style_name.split('+') {
      if let Some((name, weight)) = part.split_once('.')
        && let Ok(weight) = weight.parse::<f32>()
        && let Some(style) = self.styles.get(name)
      {
        let slice = &style[row][0];
        for (acc, v) in blended.iter_mut().zip(slice.iter()) {
          *acc += v * weight * 0.1;
        }
      }
    }
    Ok(vec![blended])
  }

  fn infer(
    &mut self,
    tokens: Vec<Vec<i64>>,
    styles: Vec<Vec<f32>>,
    speed: f32,
  ) -> Result<(Vec<f32>, Vec<f32>), String> {
    let token_shape = [tokens.len(), tokens[0].len()];
    let tokens_tensor = Tensor::from_array((
      token_shape,
      tokens.into_iter().flatten().collect::<Vec<i64>>(),
    ))
    .map_err(|e| e.to_string())?;

    let style_shape = [styles.len(), styles[0].len()];
    let style_tensor = Tensor::from_array((
      style_shape,
      styles.into_iter().flatten().collect::<Vec<f32>>(),
    ))
    .map_err(|e| e.to_string())?;

    let speed_tensor =
      Tensor::from_array(([1], vec![speed])).map_err(|e| e.to_string())?;

    let inputs: Vec<SessionInput> = vec![
      (
        Cow::Borrowed(TOKENS_KEY),
        SessionInputValue::Owned(Value::from(tokens_tensor)),
      ),
      (
        Cow::Borrowed(STYLE_KEY),
        SessionInputValue::Owned(Value::from(style_tensor)),
      ),
      (
        Cow::Borrowed(SPEED_KEY),
        SessionInputValue::Owned(Value::from(speed_tensor)),
      ),
    ];

    let outputs = self
      .session
      .run(SessionInputs::from(inputs))
      .map_err(|e| e.to_string())?;

    let (_shape, audio) = outputs[AUDIO_KEY]
      .try_extract_tensor::<f32>()
      .or_else(|_| outputs["audio"].try_extract_tensor::<f32>())
      .map_err(|_| "model output 'waveform'/'audio' not found".to_string())?;

    let (_dshape, durations) = outputs[DURATIONS_KEY]
      .try_extract_tensor::<f32>()
      .map_err(|_| "model output 'durations' not found".to_string())?;

    Ok((audio.to_vec(), durations.to_vec()))
  }
}

fn load_voices(voices_path: &Path) -> Result<VoiceStyles, String> {
  let file = File::open(voices_path)
    .map_err(|e| format!("open voices {}: {e}", voices_path.display()))?;
  let mut npz = NpzReader::new(file).map_err(|e| e.to_string())?;
  let mut map = HashMap::new();
  let names = npz.names().map_err(|e| e.to_string())?;
  for name in names {
    let data: Array3<f32> = npz.by_name(&name).map_err(|e| e.to_string())?;
    let mut rows = vec![[[0.0f32; STYLE_DIM]; 1]; 511];
    for (i, plane) in data.outer_iter().enumerate().take(511) {
      for (j, row) in plane.outer_iter().enumerate().take(1) {
        for (k, v) in row.iter().enumerate().take(STYLE_DIM) {
          rows[i][j][k] = *v;
        }
      }
    }
    map.insert(name, rows);
  }
  Ok(map)
}

fn phonemize(text: &str) -> String {
  let _guard = ESPEAK_LOCK.lock().unwrap_or_else(|e| e.into_inner());
  text_to_phonemes(text, "en-us", None, true, false)
    .unwrap_or_default()
    .join("")
}

/// Tokenize the full phrase (best prosody) and build a per-word token-span map
/// by phonemizing each word/punctuation item on its own. Ported from Kokoros.
fn tokenize_with_alignment(text: &str) -> (Vec<i64>, WordMap) {
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

fn split_words_and_punct(s: &str) -> Vec<String> {
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

fn build_alignments(
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
  let aligned_sec = cursor_frames / FRAMES_PER_SEC;
  let audio_sec = audio_len as f32 / SAMPLE_RATE as f32;
  if aligned_sec > 0.0 {
    let scale = (audio_sec / aligned_sec).clamp(0.8, 1.25);
    if (scale - 1.0).abs() > 0.005 {
      for a in &mut alignments {
        a.start_sec *= scale;
        a.end_sec *= scale;
      }
    }
  }
  alignments
}

#[cfg(test)]
mod tests {
  use super::*;

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
    assert!(
      words.len() >= 5,
      "should align several words, got {}",
      words.len()
    );
    // Monotonic non-decreasing start times.
    for pair in words.windows(2) {
      assert!(pair[1].start_sec >= pair[0].start_sec);
    }
    for w in &words {
      println!("{:>10}  {:.3}..{:.3}", w.word, w.start_sec, w.end_sec);
    }
  }
}
