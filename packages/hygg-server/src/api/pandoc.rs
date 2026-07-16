//! The `pandoc` half of `/api/v1/convert`: shelling out to the pandoc CLI for
//! DOCX/ODT/RTF and the other formats the native extractors don't cover.
//!
//! Split out of `convert.rs` to keep that module within the line budget. The
//! bounds here matter — the document is attacker-supplied and pandoc enforces
//! none of its own — so they are gathered in one place with the reasoning.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::error::{AppError, AppResult};

/// How long one `pandoc` run may take, and how much text it may emit. The
/// document is attacker-supplied — any sync-capable account can upload one —
/// and pandoc enforces neither bound itself, so without these a parser-hostile
/// or bomb document pins a blocking thread indefinitely and grows the response
/// (and the cached extraction) without limit.
const PANDOC_TIMEOUT: Duration = Duration::from_secs(60);
const PANDOC_MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

/// Convert a binary document through the `pandoc` CLI (read from stdin so no
/// temp file is needed), then justify the resulting plain text.
pub(super) fn pandoc_convert(
  bytes: &[u8],
  ext: &str,
  col: usize,
) -> AppResult<String> {
  let mut child = Command::new("pandoc")
    .args(["-f", pandoc_format(ext), "-t", "plain", "--wrap=none"])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::null())
    .spawn()
    .map_err(|_| {
      AppError::BadRequest("server cannot convert this format".to_string())
    })?;
  let mut stdin = child.stdin.take().ok_or(AppError::Internal)?;
  let mut stdout = child.stdout.take().ok_or(AppError::Internal)?;

  // Feed stdin from its own thread. pandoc can begin writing stdout before it
  // has read all of stdin, so writing everything and then reading from one
  // thread deadlocks the moment either 64 KiB pipe buffer fills. A broken pipe
  // (pandoc rejected the input and exited early) is not an error here — the
  // exit status decides that — so the write result is discarded. Dropping
  // stdin at the end of the closure signals EOF.
  let input = bytes.to_vec();
  let feeder = std::thread::spawn(move || {
    let _ = stdin.write_all(&input);
  });

  // Watchdog: kill a run that outlives its budget. Killing closes pandoc's
  // stdout, which unblocks the read below.
  let child = Arc::new(Mutex::new(child));
  let finished = Arc::new(AtomicBool::new(false));
  let (watched, watch_finished) = (Arc::clone(&child), Arc::clone(&finished));
  let watchdog = std::thread::spawn(move || {
    let deadline = Instant::now() + PANDOC_TIMEOUT;
    while Instant::now() < deadline {
      if watch_finished.load(Ordering::Relaxed) {
        return false;
      }
      std::thread::sleep(Duration::from_millis(50));
    }
    if watch_finished.load(Ordering::Relaxed) {
      return false;
    }
    if let Ok(mut child) = watched.lock() {
      let _ = child.kill();
    }
    true
  });

  // Read at most the cap plus one byte, so an over-limit run is detectable
  // without buffering the whole runaway output.
  let mut out = Vec::new();
  let read = (&mut stdout)
    .take(PANDOC_MAX_OUTPUT_BYTES as u64 + 1)
    .read_to_end(&mut out);
  finished.store(true, Ordering::Relaxed);
  let timed_out = watchdog.join().unwrap_or(false);

  let mut child = child.lock().map_err(|_| AppError::Internal)?;
  // No-op once pandoc has exited on its own; reaps it if the output cap cut the
  // read short while it was still producing.
  let _ = child.kill();
  let status = child.wait().map_err(|_| AppError::Internal)?;
  let _ = feeder.join();

  if timed_out {
    return Err(AppError::BadRequest(format!("converting .{ext} timed out")));
  }
  read.map_err(|_| AppError::Internal)?;
  if out.len() > PANDOC_MAX_OUTPUT_BYTES {
    return Err(AppError::BadRequest(format!(".{ext} produced too much text")));
  }
  if !status.success() {
    return Err(AppError::BadRequest(format!("could not convert .{ext}")));
  }
  let text = String::from_utf8_lossy(&out).into_owned();
  Ok(cli_justify::justify(&text, col).join("\n"))
}

/// Map a file extension to pandoc's input-format name (most match directly).
fn pandoc_format(ext: &str) -> &str {
  match ext {
    "htm" => "html",
    "tex" => "latex",
    "md" | "markdown" => "markdown",
    other => other,
  }
}
