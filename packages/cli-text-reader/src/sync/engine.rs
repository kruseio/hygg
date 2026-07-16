//! The background sync thread. It owns a small queue of pending progress
//! updates (coalesced to the latest per document), flushes them to the server
//! on a cadence, pulls remote changes, and backs off exponentially while
//! offline — never touching the reader's main thread. Commands frequently,
//! network rarely: it wakes every `POLL_INTERVAL` to stay responsive to
//! FlushNow / Shutdown, but only hits the network once `next_attempt` is due.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use super::client::SyncClient;
use super::types::{
  BookUpload, ProgressPayload, ReadingDayPayload, ReadingTimePayload, SyncCmd,
  SyncEvent,
};
use hygg_shared::sync::clock::SkewClock;
use hygg_shared::sync::proto;

/// Poll cadence when SSE is unavailable (no push channel — we must poll).
const FAST_INTERVAL: Duration = Duration::from_secs(3);
/// Poll cadence when SSE is healthy: pulls are driven by push notifications,
/// so this is just a safety net that also recovers silently-dropped events.
const SLOW_INTERVAL: Duration = Duration::from_secs(45);
/// After an edit, flush this soon regardless of the (possibly slow) pull
/// cadence, so pushes stay prompt without busy polling.
const FLUSH_DEBOUNCE: Duration = Duration::from_secs(2);
const POLL_INTERVAL: Duration = Duration::from_millis(500);
/// Cap exponential backoff at FAST_INTERVAL * 2^6 ≈ 3 minutes.
const MAX_BACKOFF_SHIFT: u32 = 6;

/// Pending outbound state: progress is coalesced to the latest per document;
/// annotations are an ordered list (every add/delete is a distinct op).
#[derive(Default)]
struct Pending {
  books: HashMap<String, BookUpload>,
  progress: HashMap<String, ProgressPayload>,
  annotations: Vec<proto::SyncOp>,
  /// Coalesced to the latest cumulative seconds per book.
  reading_time: HashMap<String, ReadingTimePayload>,
  /// Coalesced to the latest cumulative seconds per calendar day.
  reading_day: HashMap<String, ReadingDayPayload>,
}

impl Pending {
  fn ops(&self) -> Vec<proto::SyncOp> {
    let mut ops: Vec<proto::SyncOp> =
      self.progress.values().map(ProgressPayload::to_op).collect();
    ops.extend(self.annotations.iter().cloned());
    ops.extend(self.reading_time.values().map(ReadingTimePayload::to_op));
    ops.extend(self.reading_day.values().map(ReadingDayPayload::to_op));
    ops
  }
}

pub fn run_engine(
  client: SyncClient,
  rx: Receiver<SyncCmd>,
  tx: Sender<SyncEvent>,
  cancel: Arc<AtomicBool>,
  clock: Arc<SkewClock>,
) {
  let mut pending = Pending::default();
  let mut cursor: i64 = 0;
  let mut failures: u32 = 0;
  let mut sse_healthy = false;
  let mut next_attempt = Instant::now();
  let mut report_status = false;
  // Whether the last network cycle succeeded. We announce the *transition* into
  // failure once (so a bad/expired token or an unreachable server surfaces even
  // for background flushes the user didn't explicitly request), without
  // spamming a notification on every retry.
  let mut healthy = true;

  loop {
    if cancel.load(Ordering::Relaxed) {
      break;
    }
    match rx.recv_timeout(POLL_INTERVAL) {
      Ok(SyncCmd::Shutdown) => break,
      Ok(SyncCmd::EnqueueBook(payload)) => {
        pending.books.insert(payload.book_id.clone(), payload);
        bring_forward(&mut next_attempt, FLUSH_DEBOUNCE);
      }
      Ok(SyncCmd::EnqueueProgress(payload)) => {
        pending.progress.insert(payload.book_id.clone(), payload);
        bring_forward(&mut next_attempt, FLUSH_DEBOUNCE);
      }
      Ok(SyncCmd::EnqueueAnnotation(op)) => {
        pending.annotations.push(op);
        bring_forward(&mut next_attempt, FLUSH_DEBOUNCE);
      }
      Ok(SyncCmd::EnqueueReadingTime(payload)) => {
        pending.reading_time.insert(payload.book_id.clone(), payload);
        bring_forward(&mut next_attempt, FLUSH_DEBOUNCE);
      }
      Ok(SyncCmd::EnqueueReadingDay(payload)) => {
        pending.reading_day.insert(payload.day.clone(), payload);
        bring_forward(&mut next_attempt, FLUSH_DEBOUNCE);
      }
      Ok(SyncCmd::FlushNow { report }) => {
        next_attempt = Instant::now();
        report_status |= report;
      }
      Ok(SyncCmd::PullNow) => next_attempt = Instant::now(),
      Ok(SyncCmd::RefetchProgress) => {
        // A one-off full pull so an explicit `:server-progress` gets the
        // current server position even when it's unchanged since our
        // delta `cursor`. Read from 0 without persisting the
        // regression, so normal delta pulls still resume from the real
        // cursor. Best-effort: a network error just leaves the reader's
        // request to time out.
        if let Ok(result) = client.pull(0) {
          for row in result.progress {
            let _ = tx.send(SyncEvent::Progress(row));
          }
        }
      }
      Ok(SyncCmd::SseUp) => sse_healthy = true,
      Ok(SyncCmd::SseDown) => {
        sse_healthy = false;
        next_attempt = Instant::now();
      }
      Err(RecvTimeoutError::Disconnected) => break,
      Err(RecvTimeoutError::Timeout) => {}
    }

    if Instant::now() < next_attempt {
      continue;
    }

    match sync_once(&client, &mut pending, &mut cursor, &tx, &clock) {
      Ok(()) => {
        failures = 0;
        if !healthy {
          let _ = tx.send(SyncEvent::Connectivity { online: true });
        }
        healthy = true;
        let interval = if sse_healthy { SLOW_INTERVAL } else { FAST_INTERVAL };
        next_attempt = Instant::now() + interval;
        if report_status {
          let _ = tx.send(SyncEvent::Status {
            ok: true,
            message: "Sync complete.".to_string(),
          });
          report_status = false;
        }
      }
      Err(message) => {
        failures = (failures + 1).min(MAX_BACKOFF_SHIFT);
        next_attempt = Instant::now() + FAST_INTERVAL * (1 << failures);
        // The transition into failure raises the passive offline indicator,
        // not an overlay: an unreachable server (laptop offline, server down)
        // is a background condition the reader lives with, so it must never
        // interrupt reading. Only an explicitly requested sync reports its
        // failure as a notification.
        if healthy {
          let _ = tx.send(SyncEvent::Connectivity { online: false });
        }
        if report_status {
          let _ = tx.send(SyncEvent::Status {
            ok: false,
            message: format!("Sync failed: {message}"),
          });
          report_status = false;
        }
        healthy = false;
      }
    }
  }
}

/// This device's wall clock in epoch millis, for pairing with the server time
/// when learning the clock-skew offset. (`Instant` drives the engine's cadence
/// but isn't a wall clock, so it can't be compared to the server's.)
fn local_now_ms() -> i64 {
  std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map(|d| d.as_millis() as i64)
    .unwrap_or(0)
}

/// Pull a scheduled attempt earlier (never later), so an edit flushes promptly
/// even when the periodic cadence is slow (SSE healthy).
fn bring_forward(next_attempt: &mut Instant, within: Duration) {
  let soon = Instant::now() + within;
  if soon < *next_attempt {
    *next_attempt = soon;
  }
}

/// Push the pending batch (one request) then pull remote changes. Returns false
/// on any network error so the caller backs off and keeps the queue for retry.
fn sync_once(
  client: &SyncClient,
  pending: &mut Pending,
  cursor: &mut i64,
  tx: &Sender<SyncEvent>,
  clock: &SkewClock,
) -> Result<(), String> {
  // Upload books best-effort: a vanished or rejected document must never block
  // progress sync. A successful upload is removed from the queue; an unreadable
  // file or a permanent server rejection (4xx — e.g. the content already exists
  // under another account, or the plan's storage is full) is dropped with a
  // one-off status so the user learns why; a transient error keeps the book
  // queued to retry next cycle.
  let book_ids: Vec<String> = pending.books.keys().cloned().collect();
  for id in book_ids {
    let book = pending.books[&id].clone();
    // Full sync uploads the bytes; metadata-only registers the record with the
    // file's size but never reads or sends its contents. A read failure only
    // matters for the full path.
    let result = if book.upload_blob {
      match std::fs::read(&book.path) {
        Ok(bytes) => {
          client.upload_book(&book.book_id, &book.title, &book.format, &bytes)
        }
        Err(e) => {
          pending.books.remove(&id);
          let _ = tx.send(SyncEvent::Status {
            ok: false,
            message: format!("Couldn't read {} to upload: {e}", book.path),
          });
          continue;
        }
      }
    } else {
      let size =
        std::fs::metadata(&book.path).map(|m| m.len() as i64).unwrap_or(0);
      client.upload_book_meta(&book.book_id, &book.title, &book.format, size)
    };
    match result {
      Ok(()) => {
        pending.books.remove(&id);
      }
      Err(e) if e.permanent => {
        pending.books.remove(&id);
        let _ = tx.send(SyncEvent::Status {
          ok: false,
          message: format!(
            "Couldn\u{2019}t sync \u{201c}{}\u{201d} to the server: {}",
            book.title, e.message
          ),
        });
      }
      Err(_) => {} // transient: keep queued and retry next cycle
    }
  }

  let has_ops = !pending.progress.is_empty()
    || !pending.annotations.is_empty()
    || !pending.reading_time.is_empty()
    || !pending.reading_day.is_empty();
  if has_ops {
    client.push(&pending.ops())?;
    pending.progress.clear();
    pending.annotations.clear();
    pending.reading_time.clear();
    pending.reading_day.clear();
  }
  match client.pull(*cursor) {
    Ok(result) => {
      // Learn the clock offset from the server's reported time so the reader's
      // next op timestamp lands in the server's clock domain (skew-immune LWW).
      clock.observe(result.server_time, local_now_ms());
      *cursor = result.server_time.max(*cursor);
      for row in result.progress {
        let _ = tx.send(SyncEvent::Progress(row));
      }
      for row in result.bookmarks {
        let _ = tx.send(SyncEvent::Bookmark(row));
      }
      for row in result.highlights {
        let _ = tx.send(SyncEvent::Highlight(row));
      }
      for row in result.notes {
        let _ = tx.send(SyncEvent::Note(row));
      }
      let _ = tx.send(SyncEvent::SyncCycleComplete);
      Ok(())
    }
    Err(e) => Err(e),
  }
}
