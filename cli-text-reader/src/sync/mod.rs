//! Background sync engine. A single `std::thread` owns all network I/O via
//! `ureq`; the reader's main thread only ever does non-blocking channel
//! `send`/`try_iter`, exactly like the existing PDF-streaming and TTS workers.
//! When no server is configured `SyncHandle::spawn` returns `None` and the
//! reader runs entirely offline with zero overhead.

mod annotations;
mod client;
mod engine;
mod inbound;
mod machine;
mod sse;
mod types;

pub use machine::machine_id;

pub use annotations::{
  AnnotationOp, ServerBookmark, ServerHighlight, ServerNote,
};
pub use client::{DeviceRegistration, PullResult, SyncClient, register_device};
pub use hygg_shared::sync::proto;
pub use inbound::{RemoteBook, ServerProgress};
pub use types::{
  BookUpload, ProgressPayload, ReadingDayPayload, ReadingTimePayload, SyncEvent,
};

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use hygg_shared::sync::clock::SkewClock;

use crate::config::ServerConfig;
use types::SyncCmd;

/// Main-thread handle to the background sync engine.
pub struct SyncHandle {
  to_engine: Sender<SyncCmd>,
  from_engine: Receiver<SyncEvent>,
  cancel: Arc<AtomicBool>,
  worker: Option<JoinHandle<()>>,
  /// SSE listener thread. Detached on shutdown (it dies with the process and
  /// stops on its own within one connection lifetime once `cancel` is set), so
  /// quitting never blocks on a socket read.
  _sse: Option<JoinHandle<()>>,
  /// Clock-skew correction shared with the engine thread: the engine
  /// `observe`s the server time on every pull, and the reader stamps op
  /// timestamps through [`corrected`](SyncHandle::corrected) so
  /// last-write-wins orders this device's writes correctly against peers
  /// with differently-set clocks.
  clock: Arc<SkewClock>,
}

impl SyncHandle {
  /// Spawn the engine, or `None` if the config lacks a URL/token.
  pub fn spawn(config: &ServerConfig) -> Option<SyncHandle> {
    let client = SyncClient::from_config(config)?;
    let (to_engine, rx) = mpsc::channel();
    let (tx, from_engine) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_thread = cancel.clone();
    let clock = Arc::new(SkewClock::new());
    let clock_engine = clock.clone();
    let worker = thread::Builder::new()
      .name("hygg-sync".into())
      .spawn(move || {
        engine::run_engine(client, rx, tx, cancel_thread, clock_engine)
      })
      .ok()?;

    // SSE listener: pushes the engine to pull on server-side changes. Best
    // effort — if it can't spawn, the engine still polls on its own cadence.
    let sse_config = config.clone();
    let sse_to_engine = to_engine.clone();
    let cancel_sse = cancel.clone();
    let sse = thread::Builder::new()
      .name("hygg-sse".into())
      .spawn(move || sse::run_sse(sse_config, sse_to_engine, cancel_sse))
      .ok();

    Some(SyncHandle {
      to_engine,
      from_engine,
      cancel,
      worker: Some(worker),
      _sse: sse,
      clock,
    })
  }

  /// Map a local wall-clock millis reading into the server's clock domain, so a
  /// timestamp this reader stamps on an op sorts correctly against peers under
  /// last-write-wins. Identity until the engine's first successful pull.
  pub fn corrected(&self, local_ms: i64) -> i64 {
    self.clock.corrected(local_ms)
  }

  pub fn enqueue_progress(&self, payload: ProgressPayload) {
    let _ = self.to_engine.send(SyncCmd::EnqueueProgress(payload));
  }

  pub fn enqueue_book(&self, payload: BookUpload) {
    let _ = self.to_engine.send(SyncCmd::EnqueueBook(payload));
  }

  pub fn enqueue_annotation(&self, op: proto::SyncOp) {
    let _ = self.to_engine.send(SyncCmd::EnqueueAnnotation(op));
  }

  pub fn enqueue_reading_time(&self, payload: ReadingTimePayload) {
    let _ = self.to_engine.send(SyncCmd::EnqueueReadingTime(payload));
  }

  pub fn enqueue_reading_day(&self, payload: ReadingDayPayload) {
    let _ = self.to_engine.send(SyncCmd::EnqueueReadingDay(payload));
  }

  pub fn flush_now(&self) {
    let _ = self.to_engine.send(SyncCmd::FlushNow { report: false });
  }

  pub fn sync_now(&self) {
    let _ = self.to_engine.send(SyncCmd::FlushNow { report: true });
  }

  pub fn pull_now(&self) {
    let _ = self.to_engine.send(SyncCmd::PullNow);
  }

  /// Force a one-off full re-fetch of progress (see
  /// [`SyncCmd::RefetchProgress`]) so an explicit `:server-progress` reliably
  /// re-delivers the current server position even when it is unchanged since
  /// the delta cursor.
  pub fn refetch_progress(&self) {
    let _ = self.to_engine.send(SyncCmd::RefetchProgress);
  }

  /// Non-blocking drain of engine notifications, called once per render tick.
  pub fn drain(&self) -> Vec<SyncEvent> {
    self.from_engine.try_iter().collect()
  }

  pub fn shutdown(&mut self) {
    self.cancel.store(true, Ordering::Relaxed);
    let _ = self.to_engine.send(SyncCmd::Shutdown);
    if let Some(worker) = self.worker.take() {
      let _ = worker.join();
    }
  }
}
