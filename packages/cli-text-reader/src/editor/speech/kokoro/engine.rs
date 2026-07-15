// The Kokoro inference engine: load the timestamped ONNX model + voice styles,
// then synthesize text into 24 kHz mono f32 PCM with per-word timings.

use std::borrow::Cow;
use std::path::Path;

use ort::session::builder::SessionBuilder;
use ort::session::{Session, SessionInputValue, SessionInputs};
use ort::value::{Tensor, Value};

use super::align::{WordAlignment, build_alignments, tokenize_with_alignment};
use super::common::{
  MAX_STYLE_ROWS, MAX_TOKENS, SAMPLE_RATE, STYLE_DIM, SessionInput, VoiceStyles,
};
use super::files::load_voices;

// Timestamped Kokoro v1.0 ONNX schema.
const TOKENS_KEY: &str = "input_ids";
const STYLE_KEY: &str = "style";
const SPEED_KEY: &str = "speed";
const AUDIO_KEY: &str = "waveform";
const DURATIONS_KEY: &str = "durations";

pub(crate) struct KokoroEngine {
  session: Session,
  styles: VoiceStyles,
}

impl KokoroEngine {
  /// Load the ONNX model and voice styles from disk.
  pub(crate) fn load(
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
  pub(crate) fn synthesize(
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
