use chrono::Utc;

/// Current Unix time in milliseconds — the canonical timestamp unit across the
/// schema and sync protocol.
pub fn now_millis() -> i64 {
  Utc::now().timestamp_millis()
}
