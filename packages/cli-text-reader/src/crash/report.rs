// The pure half of the crash handler: turning a captured panic into a report,
// a one-line summary, and a file location. Nothing here touches global state,
// the terminal or the filesystem, so it's unit-testable without panicking.

use chrono::{DateTime, Utc};
use std::any::Any;
use std::backtrace::Backtrace;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::thread::ThreadId;

pub(crate) const ISSUES_URL: &str = "https://github.com/kruseio/hygg/issues";

/// Everything the panic hook learns about one panic. The backtrace is kept
/// unresolved: symbolication is the expensive part, and most panics hygg sees
/// are recovered ones (cli-pdf-to-text's per-page `catch_unwind` guards on
/// malformed PDFs) whose report is never rendered.
pub(crate) struct PanicRecord {
  pub thread_id: ThreadId,
  pub thread_name: String,
  pub message: String,
  /// `file:line:column`, the same shape Rust's own panic message uses.
  pub location: Option<String>,
  pub time: DateTime<Utc>,
  pub backtrace: Backtrace,
}

/// The panic payload as text. `panic!("literal")` carries a `&str`, a
/// formatted `panic!` a `String`; anything else (`panic_any`) has no textual
/// form, so mirror the placeholder std's default hook prints.
pub(crate) fn payload_message(payload: &(dyn Any + Send)) -> String {
  if let Some(s) = payload.downcast_ref::<&str>() {
    (*s).to_string()
  } else if let Some(s) = payload.downcast_ref::<String>() {
    s.clone()
  } else {
    "Box<dyn Any>".to_string()
  }
}

fn location_or_unknown(record: &PanicRecord) -> &str {
  record.location.as_deref().unwrap_or("<unknown location>")
}

fn write_record(out: &mut String, record: &PanicRecord) {
  // `write!` into a String cannot fail; the results are ignored rather than
  // unwrapped because this runs on the panic path.
  let _ = writeln!(out, "Time:     {}", record.time.to_rfc3339());
  let _ = writeln!(out, "Thread:   {}", record.thread_name);
  let _ = writeln!(out, "Location: {}", location_or_unknown(record));
  let _ = writeln!(out, "Message:  {}", record.message);
  let _ = writeln!(out);
  let _ = writeln!(out, "Backtrace:");
  let _ = writeln!(out, "{}", record.backtrace);
}

/// The full crash file contents: the fatal panic first, then any panics
/// recorded earlier in the session. Those earlier ones may have been caught
/// and recovered from, but when a worker thread dies and the main thread
/// panics as a consequence (a poisoned lock, a dropped channel), the earlier
/// record is the actual root cause.
pub(crate) fn format_report(
  fatal: &PanicRecord,
  earlier: &[&PanicRecord],
  version: &str,
) -> String {
  let mut out = String::new();
  let _ = writeln!(out, "hygg crash report");
  let _ = writeln!(out, "=================");
  let _ = writeln!(out, "Version:  hygg {version}");
  let _ = writeln!(
    out,
    "Platform: {} ({})",
    std::env::consts::OS,
    std::env::consts::ARCH
  );
  write_record(&mut out, fatal);
  if !earlier.is_empty() {
    let _ = writeln!(out);
    let _ = writeln!(
      out,
      "Earlier panics in this session (possibly recovered), oldest first:"
    );
    for record in earlier {
      let _ = writeln!(out);
      let _ = writeln!(out, "-----------------");
      write_record(&mut out, record);
    }
  }
  let _ = writeln!(out);
  let _ = writeln!(out, "Please report this at {ISSUES_URL}");
  out
}

/// The short message printed to the terminal once it's been restored. When
/// the crash file couldn't be written, say so and why instead of pointing at
/// a path that doesn't exist.
pub(crate) fn user_message(
  fatal: &PanicRecord,
  crash_file: Result<&Path, &str>,
) -> String {
  let details = match crash_file {
    Ok(path) => format!("Details: {}", path.display()),
    Err(e) => format!("No crash file could be written ({e})"),
  };
  format!(
    "hygg crashed: {} at {}. {details}. Please report at {ISSUES_URL}",
    fatal.message,
    location_or_unknown(fatal)
  )
}

/// hygg's log directory — crash files, the debug log and the TUI's captured
/// stderr all go here, so a bug report means attaching one folder. Highest
/// priority first: an explicit override (the `HYGG_LOG_DIR` env var, or a
/// directory injected by tests), then `<config dir>/hygg/logs` — next to the
/// config and progress files `utils::get_hygg_config_dir` manages, i.e.
/// ~/.config/hygg on Linux and ~/Library/Application Support/hygg on macOS —
/// and finally the system temp dir when there is no config dir at all.
pub(crate) fn log_dir(
  override_dir: Option<PathBuf>,
  config_dir: Option<PathBuf>,
) -> PathBuf {
  if let Some(dir) = override_dir {
    return dir;
  }
  match config_dir {
    Some(base) => base.join("hygg").join("logs"),
    None => fallback_log_dir(),
  }
}

/// Used when there's no config dir, or when writing into it failed (read-only
/// home, full disk) — a crash file in /tmp beats none at all.
pub(crate) fn fallback_log_dir() -> PathBuf {
  std::env::temp_dir().join("hygg-logs")
}

/// `crash-<UTC timestamp>-<pid>.log`. The timestamp makes `ls` sort crashes
/// chronologically; the pid keeps two hygg processes crashing in the same
/// second from overwriting each other's report.
pub(crate) fn crash_file_name(time: DateTime<Utc>, pid: u32) -> String {
  format!("crash-{}-{pid}.log", time.format("%Y%m%dT%H%M%SZ"))
}
