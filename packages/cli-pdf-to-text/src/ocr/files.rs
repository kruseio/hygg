// OCR model file management: locate, download on first use, verify, and cache.
//
// This mirrors the Kokoro TTS model fetch in
// `cli-text-reader/src/editor/speech/kokoro/files.rs`. The PaddleOCR ONNX
// models used to be embedded in the binary via `include_bytes!`; they are now
// downloaded on first use from this project's own GitHub release and cached
// under the platform cache dir, so the models are no longer checked into the
// source tree or the published crate.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use sha2::{Digest, Sha256};

// One downloadable artifact.
//
// The bytes are parsed and executed by the tract ONNX runtime, so they are
// trusted input and must be exactly what this engine was built against — hence
// the pinned `sha256`/`len`, verified before the file is allowed into the
// cache.
//
// `url` is a single, self-hosted source: an asset on this project's own GitHub
// release. There is deliberately no third-party origin and no fallback — a
// release asset is immutable once uploaded and served from infrastructure this
// project controls, so it cannot drift or vanish the way
// `huggingface.co/.../resolve/main` could.
struct Artifact {
  file_name: &'static str,
  url: &'static str,
  sha256: &'static str,
  len: u64,
}

// Published under the `ocr-models-v1.0` release. These are the raw
// (un-gzipped) PaddleOCR ONNX assets; provenance, upstream revision and the
// checksums below are recorded in
// `assets/ocr/monkt-paddleocr-onnx/MANIFEST.md`. Re-uploading the release with
// different bytes would be caught by the sha256 check — a mismatch is refused
// rather than cached — so the pin is the integrity guarantee, not the URL.
const DET: Artifact = Artifact {
  file_name: "det.onnx",
  url: "https://github.com/kruseio/hygg/releases/download/ocr-models-v1.0/det.onnx",
  sha256: "ee40e80071ba3a320d4efda75f3e22047a7d049e9bf7bcaaf9daea23fc21b935",
  len: 2_429_873,
};

const REC: Artifact = Artifact {
  file_name: "rec.onnx",
  url: "https://github.com/kruseio/hygg/releases/download/ocr-models-v1.0/rec.onnx",
  sha256: "4e16deb22c4da6468bdca539b2cd3c8687825538b67109177c47d359ab994cd7",
  len: 7_830_888,
};

const DICT: Artifact = Artifact {
  file_name: "dict.txt",
  url: "https://github.com/kruseio/hygg/releases/download/ocr-models-v1.0/dict.txt",
  sha256: "e025a66d31f327ba0c232e03f407ae8d105e1e709e7ccb3f408aa778c24e70d6",
  len: 1_416,
};

/// Directory the OCR models live in. Override with `HYGG_OCR_MODEL_DIR` (for
/// offline / air-gapped use, or to point at a pre-seeded cache); otherwise the
/// platform cache dir.
fn model_dir() -> PathBuf {
  if let Some(dir) = std::env::var_os("HYGG_OCR_MODEL_DIR") {
    return PathBuf::from(dir);
  }
  dirs::cache_dir()
    .unwrap_or_else(|| PathBuf::from("."))
    .join("hygg")
    .join("ocr")
}

/// Download one URL to `tmp`, streaming it through a SHA-256 hasher and
/// refusing anything longer than the artifact's known size. Returns the hex
/// digest of what was written; the caller compares it to the expected value.
fn download_one(
  url: &str,
  tmp: &Path,
  expect_len: u64,
) -> Result<String, String> {
  // Bound the connect and the wait for response headers so a hung or
  // unresponsive host cannot wedge the caller indefinitely on first use. No
  // global timeout — the size cap below bounds the transfer itself.
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

  // Hash as we go, and stop the moment the body runs past the known length — a
  // longer stream is a runaway response, not this file.
  let mut hasher = Sha256::new();
  let mut total = 0u64;
  let mut buf = vec![0u8; 64 * 1024];
  loop {
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

/// Serializes the cold-cache fetch in `ensure_one`. Callers arrive together far
/// more often than the "first use" framing suggests: `cargo test` runs the OCR
/// tests as threads in one process, and every one of them finds the same empty
/// cache. Without this each thread downloads all three artifacts itself.
///
/// It orders threads within one process, and nothing beyond that — two separate
/// `hygg` processes can still fetch the same artifact at once. That is left
/// correct-but-redundant rather than reaching for a file-locking dependency;
/// what makes it *correct* is the private temp name below, not this lock.
static FETCH_LOCK: Mutex<()> = Mutex::new(());

/// Distinguishes concurrent `.part` files within one process, as the pid does
/// across processes.
static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// A download-in-progress path private to this writer.
///
/// The obvious `dest.with_extension("part")` is shared, and two writers on one
/// `.part` corrupt each other in a way the SHA-256 check cannot catch: the
/// digest is computed over the received *stream*, never re-read from the file,
/// so a racer calling `File::create` on our `.part` between our last write and
/// our rename would truncate it and we would still rename it into the cache as
/// "verified". The same collision is what made the rename itself fail with
/// ENOENT once the other writer got there first. A private temp plus the atomic
/// rename means every file that reaches `dest` is one this call downloaded and
/// hashed end to end.
pub(super) fn part_path(dest: &Path) -> PathBuf {
  dest.with_extension(format!(
    "part.{}.{}",
    std::process::id(),
    TMP_SEQ.fetch_add(1, Ordering::Relaxed)
  ))
}

/// Fetch `artifact` into `dest`, refusing any download whose size or SHA-256
/// does not match. Only a verified file is moved into place; the tract runtime
/// later loads whatever lands there, so a mismatched or truncated download must
/// never be renamed into the cache.
fn fetch_verified(artifact: &Artifact, dest: &Path) -> Result<(), String> {
  if let Some(parent) = dest.parent() {
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
  }
  let tmp = part_path(dest);

  let result = match download_one(artifact.url, &tmp, artifact.len) {
    Ok(digest) if digest == artifact.sha256 => {
      return std::fs::rename(&tmp, dest).map_err(|e| e.to_string());
    }
    Ok(digest) => Err(format!(
      "{}: integrity check failed (expected sha256 {}, got {digest})",
      artifact.url, artifact.sha256
    )),
    Err(e) => Err(e),
  };
  let _ = std::fs::remove_file(&tmp);
  result
}

fn ensure_one(artifact: &Artifact) -> Result<PathBuf, String> {
  let path = model_dir().join(artifact.file_name);
  // The warm-cache path, which is every call after the first: no lock, no
  // syscall beyond the stat.
  if path.exists() {
    return Ok(path);
  }
  // The guarded value is `()`, so a downloader that panicked left nothing
  // invalid behind. Recover from the poison instead of failing every later
  // fetch in this process because one of them panicked.
  let _guard = FETCH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
  // Re-check under the lock: whoever we queued behind may have been fetching
  // this very artifact, in which case it is on disk now and we are done.
  if path.exists() {
    return Ok(path);
  }
  fetch_verified(artifact, &path)?;
  Ok(path)
}

/// Ensure the detection model, recognition model and dictionary exist locally,
/// downloading on first use. Returns the resolved paths. Network is only
/// touched when a file is missing.
pub(crate) fn ensure_ocr_models() -> Result<(PathBuf, PathBuf, PathBuf), String>
{
  let det = ensure_one(&DET)?;
  let rec = ensure_one(&REC)?;
  let dict = ensure_one(&DICT)?;
  Ok((det, rec, dict))
}
