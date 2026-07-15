//! In-process pub/sub hub backing the SSE push endpoint. Each connected client
//! subscribes to a `tokio::broadcast` channel keyed by `(tenant, user)`; when a
//! device pushes ops, the sync handler publishes a "changed" notification (the
//! new `server_time`) and every other connection for that user is woken to pull
//! immediately. Notifications are best-effort: a slow/absent client just misses
//! the ping and catches up on its next periodic pull.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::broadcast;

/// How many notifications a slow subscriber can fall behind before it is told
/// it lagged (it then does a full catch-up pull, so no data is lost).
const CHANNEL_CAPACITY: usize = 64;

/// Per-`(tenant, user)` broadcast senders. The value is the latest changed
/// `server_time`.
type Channels = Arc<Mutex<HashMap<(String, String), broadcast::Sender<i64>>>>;

#[derive(Clone, Default)]
pub struct EventHub {
  channels: Channels,
}

impl EventHub {
  /// Subscribe a connection to its user's change notifications.
  pub fn subscribe(
    &self,
    tenant: &str,
    user: &str,
  ) -> broadcast::Receiver<i64> {
    let key = (tenant.to_string(), user.to_string());
    let mut channels = self.channels.lock().expect("event hub poisoned");
    channels
      .entry(key)
      .or_insert_with(|| broadcast::channel(CHANNEL_CAPACITY).0)
      .subscribe()
  }

  /// Notify a user's other connections that something changed at `server_time`.
  /// No-op when nobody is listening (the sender is dropped once idle).
  pub fn publish(&self, tenant: &str, user: &str, server_time: i64) {
    let key = (tenant.to_string(), user.to_string());
    let channels = self.channels.lock().expect("event hub poisoned");
    if let Some(sender) = channels.get(&key) {
      let _ = sender.send(server_time);
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn subscriber_receives_published_change() {
    let hub = EventHub::default();
    let mut rx = hub.subscribe("t", "u");
    hub.publish("t", "u", 1234);
    assert_eq!(rx.recv().await.unwrap(), 1234);
  }

  #[tokio::test]
  async fn publish_is_scoped_per_user() {
    let hub = EventHub::default();
    let mut rx_u = hub.subscribe("t", "u");
    let mut other = hub.subscribe("t", "other");
    hub.publish("t", "u", 7);
    assert_eq!(rx_u.recv().await.unwrap(), 7);
    // A different user does not receive it (and a publish to nobody is a
    // no-op).
    assert!(other.try_recv().is_err());
  }

  #[tokio::test]
  async fn publish_without_subscribers_is_a_noop() {
    let hub = EventHub::default();
    hub.publish("t", "nobody", 1); // must not panic
  }
}
