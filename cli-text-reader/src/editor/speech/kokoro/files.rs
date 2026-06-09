// Model/voice file management: locate, download on first use, and load the
// per-voice style table from the npz voices file.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use ndarray::Array3;
use ndarray_npy::NpzReader;

use super::common::{STYLE_DIM, VoiceStyles};

const MODEL_URL: &str = "https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX-timestamped/resolve/main/onnx/model.onnx";
const VOICES_URL: &str = "https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.0/voices-v1.0.bin";

/// Directory the model + voices live in. Override with `HYGG_TTS_MODEL_DIR`
/// (for offline / air-gapped use); otherwise the platform cache dir.
pub(crate) fn model_dir() -> PathBuf {
  if let Some(dir) = std::env::var_os("HYGG_TTS_MODEL_DIR") {
    return PathBuf::from(dir);
  }
  dirs::cache_dir()
    .unwrap_or_else(|| PathBuf::from("."))
    .join("hygg")
    .join("tts")
}

pub(crate) fn model_path() -> PathBuf {
  model_dir().join("kokoro-v1.0-timestamped.onnx")
}

pub(crate) fn voices_path() -> PathBuf {
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
pub(crate) fn ensure_models(
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

pub(crate) fn load_voices(voices_path: &Path) -> Result<VoiceStyles, String> {
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
