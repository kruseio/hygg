//! Background SSE listener. Holds a long-lived `GET /api/v1/events` connection
//! and, on each server `data:` event, nudges the engine to pull immediately
//! (near-instant cross-device updates instead of waiting for the periodic
//! poll). It tells the engine when the stream is up (so the engine can slow its
//! safety-net polling) and down (so it falls back to fast polling), and
//! reconnects with backoff. Runs only when a server is configured; entirely
//! separate from the reader's main thread.

use std::io::{BufRead, BufReader};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::time::Duration;

use hygg_shared::sync::headers::{MACHINE_ID_HEADER, USER_HEADER};
use hygg_shared::sync::proto::events::Changed;

use super::types::SyncCmd;
use crate::config::ServerConfig;

/// Bound each connection's lifetime so a wedged socket can't stall shutdown
/// detection longer than this; the server's keep-alive holds it open meanwhile.
const CONNECT_LIFETIME: Duration = Duration::from_secs(20);
const MAX_BACKOFF_SECS: u64 = 60;

pub fn run_sse(
  config: ServerConfig,
  to_engine: Sender<SyncCmd>,
  cancel: Arc<AtomicBool>,
) {
  let (Some(url), Some(username), Some(token)) =
    (config.server_url, config.username, config.api_token)
  else {
    return;
  };
  let base = url.trim_end_matches('/').to_string();
  let machine_id = super::machine::machine_id();
  let mut backoff = 1u64;

  while !cancel.load(Ordering::Relaxed) {
    let streamed =
      stream_once(&base, &username, &token, &machine_id, &to_engine, &cancel);
    // The stream ended (timeout, disconnect, or error): fall back to polling.
    let _ = to_engine.send(SyncCmd::SseDown);
    if cancel.load(Ordering::Relaxed) {
      break;
    }
    backoff = if streamed { 1 } else { (backoff * 2).min(MAX_BACKOFF_SECS) };
    sleep_cancellable(backoff, &cancel);
  }
}

/// Connect once and stream events until the connection ends. Returns whether a
/// connection was established (false = connect failed, so the caller backs
/// off).
fn stream_once(
  base: &str,
  username: &str,
  token: &str,
  machine_id: &str,
  to_engine: &Sender<SyncCmd>,
  cancel: &AtomicBool,
) -> bool {
  let agent = ureq::Agent::config_builder()
    .timeout_global(Some(CONNECT_LIFETIME))
    .build()
    .new_agent();
  let url = format!("{base}/api/v1/events");
  let response = agent
    .get(&url)
    .header("Authorization", &format!("Bearer {token}"))
    .header(USER_HEADER, username)
    .header(MACHINE_ID_HEADER, machine_id)
    .header("Accept", "text/event-stream")
    .call();
  let response = match response {
    Ok(response) => response,
    Err(_) => return false,
  };

  // Connected: slow the engine's polling and pull once to catch up on anything
  // that changed before we subscribed.
  let _ = to_engine.send(SyncCmd::SseUp);
  let _ = to_engine.send(SyncCmd::PullNow);

  let reader = BufReader::new(response.into_body().into_reader());
  for line in reader.lines() {
    if cancel.load(Ordering::Relaxed) {
      return true;
    }
    match line {
      Ok(line) if changed_event(&line).is_some() => {
        let _ = to_engine.send(SyncCmd::PullNow);
      }
      Ok(_) => {} // keep-alive comment or other SSE field — ignore
      Err(_) => break, // timeout or disconnect
    }
  }
  true
}

/// Parse an SSE `data:` line into the typed `changed` event. Comments (`:`),
/// the `event:` line, and blanks carry no data and yield `None`.
fn changed_event(line: &str) -> Option<Changed> {
  let payload = line.strip_prefix("data:")?.trim();
  serde_json::from_str(payload).ok()
}

fn sleep_cancellable(secs: u64, cancel: &AtomicBool) {
  for _ in 0..(secs * 4) {
    if cancel.load(Ordering::Relaxed) {
      return;
    }
    std::thread::sleep(Duration::from_millis(250));
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn only_typed_data_lines_count_as_events() {
    assert!(changed_event("data: {\"server_time\":7}").is_some());
    assert!(changed_event("data:{\"server_time\":0}").is_some());
    assert!(changed_event(":ping").is_none()); // keep-alive comment
    assert!(changed_event("event:changed").is_none()); // the event-name line
    assert!(changed_event("data: not-json").is_none());
    assert!(changed_event("").is_none());
  }
}
