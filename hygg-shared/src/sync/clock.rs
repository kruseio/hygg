//! Cross-device clock-skew correction for last-write-wins ordering.
//!
//! Reading progress is merged on the server by last-write-wins on a
//! client-supplied `updated_at` (see the sync `push`/`progress` upsert). That
//! is only correct if every device's timestamps are comparable — but two
//! machines' wall clocks routinely differ by seconds, so a device whose clock
//! runs ahead would always win, and opening a document on a peer could resume
//! the *older* position (the "opens a page off" bug).
//!
//! [`SkewClock`] fixes this without a protocol change: every server response
//! carries the server's current time, so a client `observe`s it against its own
//! wall clock to learn the offset, then stamps every op via [`corrected`] —
//! yielding a timestamp in the *server's* clock domain. All clients then order
//! by one clock, so skew can't reorder writes. Unlike stamping at the server on
//! receipt, this preserves the real order of edits made while offline (a
//! position read an hour earlier keeps its earlier timestamp), because the
//! offset shifts the local event time rather than replacing it with now.
//!
//! [`corrected`]: SkewClock::corrected

use std::sync::atomic::{AtomicI64, Ordering};

/// Learns the offset between this device's wall clock and the server's, and
/// applies it so locally-created timestamps sort correctly against every peer.
///
/// Cheap and lock-free (a single relaxed atomic): the offset is a hint, not a
/// synchronization point, so a slightly stale read never breaks correctness —
/// the next server response refreshes it. Shareable across threads (`Sync`), so
/// the CLI's engine thread can `observe` while its reader thread stamps ops.
#[derive(Debug, Default)]
pub struct SkewClock {
  /// `server_ms - local_ms` at the last observation; `0` (identity) until the
  /// first server response is seen.
  offset_ms: AtomicI64,
}

impl SkewClock {
  pub const fn new() -> Self {
    Self { offset_ms: AtomicI64::new(0) }
  }

  /// Record the offset from a server response: `server_ms` is the time the
  /// server reported, `local_ms` this device's wall clock at (about) the same
  /// instant. Network latency adds at most the round-trip to the error, which
  /// is negligible next to the multi-second skews this guards against.
  pub fn observe(&self, server_ms: i64, local_ms: i64) {
    self.offset_ms.store(server_ms - local_ms, Ordering::Relaxed);
  }

  /// Map a local wall-clock millis reading into the server's clock domain.
  /// Identity until the first [`observe`](Self::observe).
  pub fn corrected(&self, local_ms: i64) -> i64 {
    local_ms.saturating_add(self.offset_ms.load(Ordering::Relaxed))
  }

  /// The current offset in millis (`server - local`), for diagnostics/tests.
  pub fn offset_ms(&self) -> i64 {
    self.offset_ms.load(Ordering::Relaxed)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn identity_until_observed() {
    let c = SkewClock::new();
    assert_eq!(c.corrected(1_000), 1_000);
    assert_eq!(c.offset_ms(), 0);
  }

  #[test]
  fn corrects_local_into_server_domain() {
    let c = SkewClock::new();
    // This device's clock is 5s behind the server: local 1_000, server 6_000.
    c.observe(6_000, 1_000);
    assert_eq!(c.offset_ms(), 5_000);
    // A later local reading is shifted by the same offset, preserving order.
    assert_eq!(c.corrected(2_000), 7_000);
  }

  #[test]
  fn offline_events_keep_relative_order() {
    // Two devices whose clocks differ by 10s, each stamping an offline read.
    let ahead = SkewClock::new();
    ahead.observe(0, 10_000); // ahead device: local 10s > server
    let behind = SkewClock::new();
    behind.observe(0, 0); // behind device: local == server
    // Real order: `behind` read first (server-time 100), `ahead` later (200).
    let behind_ts = behind.corrected(100);
    let ahead_ts = ahead.corrected(10_200);
    assert!(behind_ts < ahead_ts, "earlier real event must sort earlier");
  }
}
