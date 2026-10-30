// Opt-in debug log (`HYGG_DEBUG=1`), written to `<log dir>/debug.log`.
//
// Nothing here writes to stderr. It used to echo every line there (and print
// "DEBUG: init_debug_logging called" on every run, debug mode or not), which
// was the stderr noise the TUI then had to hide by sending stderr to
// /dev/null — taking panic messages down with it (#99). The file is the
// single destination; `CrashGuard` tells the user where it is on exit.

use std::fs::OpenOptions;
use std::io::{IsTerminal, Write};
use std::path::PathBuf;
use std::sync::Mutex;

static DEBUG_FILE: Mutex<Option<std::fs::File>> = Mutex::new(None);

/// Where `init_debug_logging` puts the log, alongside the crash files.
fn debug_log_file_path() -> PathBuf {
  crate::crash::log_dir().join("debug.log")
}

pub fn debug_enabled() -> bool {
  std::env::var("HYGG_DEBUG").is_ok()
}

/// The debug log's path, once `init_debug_logging` has actually opened it —
/// so callers only point users at a file that exists.
pub fn debug_log_path() -> Option<PathBuf> {
  let opened = DEBUG_FILE.try_lock().map(|f| f.is_some()).unwrap_or(false);
  opened.then(debug_log_file_path)
}

/// Appends a pre-formatted block (a panic report) to the debug log. Unlike
/// `debug_log` it never blocks on the log mutex: it runs inside the panic
/// hook, where the panicking thread may be the one holding the lock.
pub fn append_to_debug_log(block: &str) {
  if !debug_enabled() {
    return;
  }
  if let Ok(mut debug_file) = DEBUG_FILE.try_lock()
    && let Some(ref mut file) = *debug_file
  {
    let _ = writeln!(file, "{block}");
    let _ = file.flush();
  }
}

/// Opens (truncating) the debug log when `HYGG_DEBUG` is set; a no-op
/// otherwise. Idempotent: `hygg` calls it first thing so the startup lines
/// below describe the launch, and the reader's entry points call it again,
/// which must not wipe what's already there.
pub fn init_debug_logging() -> Result<(), Box<dyn std::error::Error>> {
  if !debug_enabled() {
    return Ok(());
  }
  let mut debug_file = DEBUG_FILE
    .lock()
    .map_err(|e| format!("Failed to acquire debug file mutex: {e}"))?;
  if debug_file.is_some() {
    return Ok(());
  }

  let debug_file_path = debug_log_file_path();
  if let Some(dir) = debug_file_path.parent() {
    std::fs::create_dir_all(dir)?;
  }
  let mut file = OpenOptions::new()
    .create(true)
    .truncate(true) // One log per run: the previous one is stale.
    .write(true)
    .open(&debug_file_path)?;
  write!(file, "{}", startup_header(&debug_file_path))?;
  file.flush()?;
  *debug_file = Some(file);
  Ok(())
}

/// The environment facts most bug reports end up asking for, so the debug log
/// answers them up front.
fn startup_header(debug_file_path: &std::path::Path) -> String {
  let env =
    |key: &str| std::env::var(key).unwrap_or_else(|_| "<unset>".to_string());
  let size = match crossterm::terminal::size() {
    Ok((cols, rows)) => format!("{cols}x{rows}"),
    Err(e) => format!("unavailable ({e})"),
  };
  let log_dir = crate::crash::log_dir();
  let mut out = String::new();
  let mut line = |s: String| {
    out.push_str(&s);
    out.push('\n');
  };
  line("=== HYGG DEBUG LOG STARTED ===".to_string());
  line(format!(
    "Timestamp: {}",
    chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
  ));
  line(format!("Version: hygg {}", env!("CARGO_PKG_VERSION")));
  line(format!(
    "Platform: {} {}",
    std::env::consts::OS,
    std::env::consts::ARCH
  ));
  line(format!(
    "Terminal: {size}, stdin tty={}, stdout tty={}",
    std::io::stdin().is_terminal(),
    std::io::stdout().is_terminal()
  ));
  for key in ["TERM", "COLORTERM", "TERM_PROGRAM", "LANG", "LC_ALL"] {
    line(format!("Env: {key}={}", env(key)));
  }
  line(format!("Env: HYGG_DEBUG={}", env("HYGG_DEBUG")));
  line(format!("Debug log: {}", debug_file_path.display()));
  line(format!(
    "Crash files: {}",
    log_dir.join("crash-<time>-<pid>.log").display()
  ));
  line(format!(
    "Captured stderr: {}",
    crate::crash::stderr_log_path().display()
  ));
  line("===============================".to_string());
  out
}

pub fn debug_log(module: &str, message: &str) {
  if !debug_enabled() {
    return;
  }
  let timestamp = chrono::Utc::now().format("%H:%M:%S%.3f");
  if let Ok(mut debug_file) = DEBUG_FILE.lock()
    && let Some(ref mut file) = *debug_file
  {
    let _ = writeln!(file, "[{timestamp}] [{module}] {message}");
    let _ = file.flush(); // Ensure immediate write
  }
}

pub fn debug_log_event(module: &str, event: &str, details: &str) {
  debug_log(module, &format!("EVENT: {event} | {details}"));
}

pub fn debug_log_state(module: &str, state_name: &str, state_value: &str) {
  debug_log(module, &format!("STATE: {state_name} = {state_value}"));
}

pub fn debug_log_error(module: &str, error: &str) {
  debug_log(module, &format!("ERROR: {error}"));
}

pub fn flush_debug_log() {
  if let Ok(mut debug_file) = DEBUG_FILE.lock()
    && let Some(ref mut file) = *debug_file
  {
    let _ = file.flush();
  }
}
