//! The `GET /api/v1/events` server-sent-events stream. The server emits a
//! single event kind; the client treats any [`Changed`] as "pull now".

use serde::{Deserialize, Serialize};

/// The SSE `event:` name the server emits when a peer device pushes.
pub const CHANGED: &str = "changed";

/// The `data:` payload of a [`CHANGED`] event. `server_time` is the push's
/// server timestamp (epoch millis), or `0` when emitted to recover a lagged
/// subscriber.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Changed {
  pub server_time: i64,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn changed_event_round_trips() {
    let value = serde_json::to_value(Changed { server_time: 9 }).unwrap();
    assert_eq!(value["server_time"], 9);
    let back: Changed = serde_json::from_value(value).unwrap();
    assert_eq!(back.server_time, 9);
  }
}
