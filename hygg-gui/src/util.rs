//! Tiny helpers shared by the app.

/// Open a URL in the user's default browser (best-effort; a failure is silently
/// ignored — the About / Credits links are non-essential). Uses the platform's
/// default opener: `open` (macOS), `xdg-open` (Linux), `cmd /C start`
/// (Windows).
pub fn open_url(url: &str) {
  #[cfg(target_os = "macos")]
  {
    let _ = std::process::Command::new("open").arg(url).spawn();
  }
  #[cfg(target_os = "linux")]
  {
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
  }
  #[cfg(target_os = "windows")]
  {
    let _ =
      std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn();
  }
  #[cfg(not(any(
    target_os = "macos",
    target_os = "linux",
    target_os = "windows"
  )))]
  {
    let _ = url;
  }
}

/// Epoch milliseconds, from the platform clock. Used for `added_at` /
/// `updated_at` timestamps so a document sorts and reports "last read" the same
/// way it does in the CLI, PWA and server.
pub fn now_ms() -> f64 {
  use std::time::{SystemTime, UNIX_EPOCH};
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map(|d| d.as_millis() as f64)
    .unwrap_or(0.0)
}

/// Compact reading-time total (`3h 12m`, `48m`, `30s`).
pub fn fmt_duration(seconds: f64) -> String {
  let s = seconds.max(0.0) as u64;
  let (h, m, sec) = (s / 3600, (s % 3600) / 60, s % 60);
  if h > 0 {
    format!("{h}h {m}m")
  } else if m > 0 {
    format!("{m}m")
  } else {
    format!("{sec}s")
  }
}

/// "Last read" phrasing from an epoch-millis timestamp. Empty when never read.
pub fn fmt_relative(updated_at_ms: f64) -> String {
  if updated_at_ms <= 0.0 {
    return String::new();
  }
  let delta = (now_ms() - updated_at_ms).max(0.0) / 1000.0;
  if delta < 60.0 {
    "just now".to_string()
  } else if delta < 3600.0 {
    format!("{}m ago", (delta / 60.0) as u64)
  } else if delta < 86_400.0 {
    format!("{}h ago", (delta / 3600.0) as u64)
  } else {
    format!("{}d ago", (delta / 86_400.0) as u64)
  }
}
