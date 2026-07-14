//! Skew-corrected wall clock for sync ordering. Wraps a process-global
//! [`SkewClock`] over the browser's `Date.now()` so every progress timestamp
//! this device stamps — the pushed op, the local last-write-wins baseline —
//! lands in the *server's* clock domain. Without this, a browser whose clock
//! runs ahead of the CLI would always win last-write-wins and a peer could
//! resume the older position. The offset is refreshed from the `server_time`
//! every pull/push response carries (see [`observe`]).

use hygg_shared::sync::clock::SkewClock;

// Single-threaded (wasm): one global instance for the whole app.
static CLOCK: SkewClock = SkewClock::new();

/// This device's raw wall clock in epoch millis (uncorrected). Use for local
/// durations (throttles), never for cross-device ordering.
pub fn local_ms() -> f64 {
  js_sys::Date::now()
}

/// Learn the clock offset from a server response's `server_time` (epoch
/// millis).
pub fn observe(server_time_ms: i64) {
  CLOCK.observe(server_time_ms, local_ms() as i64);
}

/// The current time in the server's clock domain (epoch millis) — the value to
/// stamp on progress ops and to seed the last-write-wins baseline with.
pub fn now_ms() -> f64 {
  CLOCK.corrected(local_ms() as i64) as f64
}
