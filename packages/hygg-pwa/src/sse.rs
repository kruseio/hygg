//! Live server push over Server-Sent Events — the browser equivalent of the
//! CLI's raw HTTP `changed` stream
//! (`packages/cli-text-reader/src/sync/sse.rs`), so a peer's progress or
//! library change reaches this device near-instantly instead of waiting for a
//! poll.
//!
//! The server emits one event kind, `changed`; the payload is empty of meaning
//! (`{server_time}`), so a listener just *pulls* to catch up. A browser
//! `EventSource` can't set request headers, so the device credential rides in
//! the query string — the server's `/api/v1/events` accepts it there. The
//! `EventSource` reconnects on its own after a drop, so there's no backoff to
//! manage here.

use hygg_shared::sync::proto::events::CHANGED;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{EventSource, MessageEvent};

use crate::sync::Creds;

/// A live subscription to the `changed` stream. Closes the underlying
/// `EventSource` (and frees its listener) on drop, so keep it alive for exactly
/// as long as you want the stream — typically by moving it into an `on_cleanup`
/// guard tied to a component's lifetime.
pub struct Events {
  source: EventSource,
  _on_changed: Closure<dyn FnMut(MessageEvent)>,
}

impl Drop for Events {
  fn drop(&mut self) {
    self.source.close();
  }
}

/// Open the `changed` stream for these credentials, calling `on_changed` on
/// every event. Returns `None` if the browser can't open the connection at all
/// (bad URL) — a *dropped* connection is retried automatically by the browser,
/// so callers don't handle reconnection.
pub fn connect(
  creds: &Creds,
  on_changed: impl Fn() + 'static,
) -> Option<Events> {
  let enc = |s: &str| String::from(js_sys::encode_uri_component(s));
  let url = format!(
    "{}/api/v1/events?token={}&user={}&machine={}",
    creds.server.trim_end_matches('/'),
    enc(&creds.token),
    enc(&creds.username),
    enc(&creds.machine_id),
  );
  let source = EventSource::new(&url).ok()?;
  // Named SSE events are only delivered to matching `addEventListener`s (never
  // to `onmessage`), so listen for `changed` specifically.
  let on_changed =
    Closure::<dyn FnMut(MessageEvent)>::new(move |_ev: MessageEvent| {
      on_changed();
    });
  source
    .add_event_listener_with_callback(
      CHANGED,
      on_changed.as_ref().unchecked_ref(),
    )
    .ok()?;
  Some(Events { source, _on_changed: on_changed })
}
