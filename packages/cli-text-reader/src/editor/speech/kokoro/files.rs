// Model/voice file management: locate, download on first use, and load the
// per-voice style table from the npz voices file.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use ndarray::Array3;
use ndarray_npy::NpzReader;
use sha2::{Digest, Sha256};

use super::common::{STYLE_DIM, VoiceStyles};

// One downloadable artifact.
//
// The bytes are parsed and executed by ONNX Runtime, so they are trusted input
// and must be exactly what this reader was built against — hence the pinned
// `sha256`/`len`, verified before the file is allowed into the cache.
//
// `url` is a single, self-hosted source: an asset on this project's own GitHub
// release. There is deliberately no third-party origin and no fallback — a
// release asset is immutable once uploaded and served from infrastructure this
// project controls, so it cannot drift or vanish out from under a release the
// way `huggingface.co/.../resolve/main` could. It remains a runtime download
// (the file is far too large to bundle into the published crate, and too large
// for the metered LFS bandwidth an in-tree copy would draw on), just one that
// only ever talks to this project's own release.
struct Artifact {
  file_name: &'static str,
  url: &'static str,
  sha256: &'static str,
  len: u64,
}

// Published under the `tts-models-v1.0` release; the assets keep the file names
// below. Re-uploading the same release with different bytes would be caught by
// the sha256 check — a mismatch is refused rather than cached — so the pin is
// the integrity guarantee, not the URL.
const MODEL: Artifact = Artifact {
  file_name: "kokoro-v1.0-timestamped.onnx",
  url: "https://github.com/kruseio/hygg/releases/download/tts-models-v1.0/kokoro-v1.0-timestamped.onnx",
  sha256: "651ea8291843a92276a4a003581a215cb07d15e47dde6fcfb1b768f9a1682054",
  len: 325_532_171,
};

const VOICES: Artifact = Artifact {
  file_name: "voices-v1.0.bin",
  url: "https://github.com/kruseio/hygg/releases/download/tts-models-v1.0/voices-v1.0.bin",
  sha256: "bca610b8308e8d99f32e6fe4197e7ec01679264efed0cac9140fe9c29f1fbf7d",
  len: 28_214_398,
};

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
  model_dir().join(MODEL.file_name)
}

pub(crate) fn voices_path() -> PathBuf {
  model_dir().join(VOICES.file_name)
}

/// Download one URL to `tmp`, streaming it through a SHA-256 hasher and
/// refusing anything longer than the artifact's known size. Returns the hex
/// digest of what was written; the caller compares it to the expected value.
fn download_one(
  url: &str,
  tmp: &Path,
  expect_len: u64,
  cancel: &AtomicBool,
) -> Result<String, String> {
  // A configured agent, like build_agent() on the sync side, rather than the
  // default global one that waits forever: bound the connect and the wait for
  // response headers so a hung or unresponsive host cannot wedge the narration
  // worker indefinitely on first use. No global timeout — a legitimate ~310 MiB
  // body over a slow link is legitimately slow, and the size cap plus the
  // cancel flag are what bound the transfer itself.
  let agent = ureq::Agent::config_builder()
    .timeout_connect(Some(Duration::from_secs(30)))
    .timeout_recv_response(Some(Duration::from_secs(60)))
    .build()
    .new_agent();
  let body = agent
    .get(url)
    .call()
    .map_err(|e| format!("download {url} failed: {e}"))?
    .into_body();
  let mut reader = body.into_reader();
  let mut file = File::create(tmp).map_err(|e| e.to_string())?;

  // Copy in chunks (rather than io::copy) so a stop request aborts this
  // multi-hundred-MB download promptly instead of running to completion on a
  // detached thread after the user has already pressed a key. Hash as we go,
  // and stop the moment the body runs past the known length — a longer stream
  // is a runaway response, not this file.
  let mut hasher = Sha256::new();
  let mut total = 0u64;
  let mut buf = vec![0u8; 64 * 1024];
  loop {
    if cancel.load(Ordering::Relaxed) {
      return Err("cancelled".to_string());
    }
    let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
    if n == 0 {
      break;
    }
    total += n as u64;
    if total > expect_len {
      return Err(format!(
        "{url}: longer than the expected {expect_len} bytes"
      ));
    }
    hasher.update(&buf[..n]);
    file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
  }
  file.flush().map_err(|e| e.to_string())?;
  Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Fetch `artifact` into `dest`, trying each source in turn and refusing any
/// download whose size or SHA-256 does not match. Only a verified file is moved
/// into place; ONNX Runtime later loads whatever lands there, so a mismatched
/// or truncated download must never be renamed into the cache.
fn fetch_verified(
  artifact: &Artifact,
  dest: &Path,
  cancel: &AtomicBool,
) -> Result<(), String> {
  if let Some(parent) = dest.parent() {
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
  }
  let tmp = dest.with_extension("part");

  let result = match download_one(artifact.url, &tmp, artifact.len, cancel) {
    Ok(digest) if digest == artifact.sha256 => {
      return std::fs::rename(&tmp, dest).map_err(|e| e.to_string());
    }
    Ok(digest) => Err(format!(
      "{}: integrity check failed (expected sha256 {}, got {digest})",
      artifact.url, artifact.sha256
    )),
    Err(e) => Err(e),
  };
  // A wrong or partial download must never be renamed into the cache, where
  // ONNX Runtime would load it unchecked next time.
  let _ = std::fs::remove_file(&tmp);
  result
}

/// Ensure the model + voices exist locally, downloading on first use. Returns
/// the resolved paths. Network is only touched when a file is missing.
pub(crate) fn ensure_models(
  cancel: &AtomicBool,
) -> Result<(PathBuf, PathBuf), String> {
  let (model, voices) = (model_path(), voices_path());
  if !model.exists() {
    fetch_verified(&MODEL, &model, cancel)?;
  }
  if !voices.exists() {
    fetch_verified(&VOICES, &voices, cancel)?;
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
