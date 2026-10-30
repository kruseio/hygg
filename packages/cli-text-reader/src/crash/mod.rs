// Crash reporting for the TUI (#99).
//
// While the reader owns the screen, stderr is useless for a panic message: it
// is either redirected or drawn over by the alternate screen, and raw mode is
// left on because the reader's cleanup is skipped on unwind. A crash used to
// surface as nothing but "exit code 101".
//
// The work is split in two on purpose:
//
// - The panic hook only *records* the panic (message, location, thread,
//   unresolved backtrace). A hook can't tell whether the panic will be caught,
//   and hygg catches plenty: cli-pdf-to-text wraps every page extraction in
//   `catch_unwind`, and some of that runs on the main thread while the TUI is
//   up (e.g. re-rendering OCR-enhanced pages). Tearing down the terminal or
//   writing a crash file from the hook would wreck a session that was about to
//   recover.
//
// - `CrashGuard`, held for the whole of `main`, does the rest from its `Drop`.
//   Drop only runs during an unwind that has escaped every `catch_unwind` below
//   it, so by then the panic is known to be fatal: restore the terminal, write
//   the crash file, and tell the user where it is.

mod report;
#[cfg(test)]
mod tests;

use crossterm::{cursor, execute, terminal};
use report::{
  PanicRecord, crash_file_name, fallback_log_dir, format_report,
  log_dir as resolve_log_dir, payload_message, user_message,
};
use std::backtrace::Backtrace;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

/// Bounded so a PDF that trips a recovered panic on every page can't grow
/// this without limit; the most recent few are the ones that explain a crash.
const MAX_RECORDS: usize = 8;

static RECORDS: Mutex<Vec<PanicRecord>> = Mutex::new(Vec::new());

/// Where hygg writes crash files, the debug log and the TUI's captured stderr
/// (see `report::log_dir`). Not created here; writers create it on demand.
pub fn log_dir() -> PathBuf {
  resolve_log_dir(
    std::env::var_os("HYGG_LOG_DIR").map(PathBuf::from),
    dirs::config_dir(),
  )
}

/// Where `hygg` sends stderr while the TUI owns the screen, so output from
/// libraries (TTS audio backends, espeak-ng) neither corrupts the display nor
/// disappears.
pub fn stderr_log_path() -> PathBuf {
  log_dir().join("stderr.log")
}

/// Installs the crash-recording panic hook and returns the guard that turns a
/// fatal panic into a crash report. Call once, early in `main`, and bind the
/// guard to a named variable (`let _crash_guard = ...`): `let _ = ...` drops
/// it immediately and nothing will be reported.
///
/// The previous hook (normally std's default) still runs after ours. Under
/// the TUI `hygg` points stderr at `stderr.log`, so its message lands there
/// instead of over the screen; on the non-TUI paths stderr is live and it
/// keeps the familiar Rust panic message (and `RUST_BACKTRACE` behaviour), and
/// under `cargo test` it keeps libtest's captured panic output.
#[must_use = "bind the guard for the whole of main: `let _crash_guard = ...`"]
pub fn install_panic_hook() -> CrashGuard {
  let previous = std::panic::take_hook();
  std::panic::set_hook(Box::new(move |info| {
    record_panic(info);
    previous(info);
  }));
  CrashGuard { dir: None }
}

/// See the module comment. When dropped normally it only prints the debug
/// log's path (debug mode only).
pub struct CrashGuard {
  /// Injected log directory; `None` means `log_dir()`.
  dir: Option<PathBuf>,
}

impl CrashGuard {
  #[cfg(test)]
  fn with_dir(dir: PathBuf) -> Self {
    Self { dir: Some(dir) }
  }
}

impl Drop for CrashGuard {
  fn drop(&mut self) {
    if std::thread::panicking() {
      report_fatal_panic(self.dir.clone());
    } else if let Some(path) = crate::debug::debug_log_path() {
      // Normal exit in debug mode: say which file to attach to an issue.
      // stderr, not stdout, so `HYGG_DEBUG=1 hygg book.pdf > out.txt` keeps a
      // clean output file; by now the TUI has handed stderr back.
      crate::debug::flush_debug_log();
      let _ = writeln!(std::io::stderr(), "hygg debug log: {}", path.display());
    }
  }
}

fn record_panic(info: &std::panic::PanicHookInfo<'_>) {
  let thread = std::thread::current();
  let record = PanicRecord {
    thread_id: thread.id(),
    thread_name: thread.name().unwrap_or("<unnamed>").to_string(),
    message: payload_message(info.payload()),
    location: info
      .location()
      .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column())),
    time: chrono::Utc::now(),
    // force_capture: RUST_BACKTRACE is never set by end users, and a crash
    // report without a backtrace is what made #99 undiagnosable.
    backtrace: Backtrace::force_capture(),
  };

  // The debug log gets every panic, recovered or not — it's the place to see
  // a malformed PDF page tripping the extractor. Rendering the backtrace is
  // only paid for when debug logging is on.
  if crate::debug::debug_enabled() {
    crate::debug::append_to_debug_log(&format!(
      "=== PANIC (may be recovered) ===\n{}",
      format_report(&record, &[], env!("CARGO_PKG_VERSION"))
    ));
  }

  let mut records = RECORDS.lock().unwrap_or_else(PoisonError::into_inner);
  if records.len() >= MAX_RECORDS {
    records.remove(0);
  }
  records.push(record);
}

fn report_fatal_panic(dir_override: Option<PathBuf>) {
  // Terminal first, so everything printed below lands on the normal screen
  // with working newlines.
  restore_terminal();

  let mut records = std::mem::take(
    &mut *RECORDS.lock().unwrap_or_else(PoisonError::into_inner),
  );
  // The panic now unwinding is the most recent one recorded on this thread.
  let current = std::thread::current().id();
  let fatal = match records.iter().rposition(|r| r.thread_id == current) {
    Some(i) => records.remove(i),
    // Only reachable if the hook was replaced after install.
    None => PanicRecord {
      thread_id: current,
      thread_name: std::thread::current()
        .name()
        .unwrap_or("<unnamed>")
        .to_string(),
      message: "unknown panic (crash hook was not installed)".to_string(),
      location: None,
      time: chrono::Utc::now(),
      backtrace: Backtrace::disabled(),
    },
  };
  let earlier: Vec<&PanicRecord> = records.iter().collect();
  let report = format_report(&fatal, &earlier, env!("CARGO_PKG_VERSION"));

  let dir = dir_override.unwrap_or_else(log_dir);
  let name = crash_file_name(fatal.time, std::process::id());
  let written = write_crash_file(&dir, &name, &report)
    .or_else(|_| write_crash_file(&fallback_log_dir(), &name, &report))
    .map_err(|e| e.to_string());

  if crate::debug::debug_enabled() {
    crate::debug::append_to_debug_log(&format!(
      "=== FATAL PANIC ===\n{report}"
    ));
    crate::debug::flush_debug_log();
  }

  let summary =
    user_message(&fatal, written.as_deref().map_err(|e| e.as_str()));
  // Write errors are ignored throughout: a panic while already panicking
  // aborts the process and would lose the exit code and the report.
  let mut stdout = std::io::stdout();
  if stdout.is_terminal() {
    let _ = writeln!(stdout, "{summary}");
    if crate::debug::debug_enabled() {
      // Debug mode is for bug reports, so put the whole thing on screen too.
      let _ = writeln!(stdout, "\n{report}");
      if let Some(path) = crate::debug::debug_log_path() {
        let _ = writeln!(stdout, "Debug log: {}", path.display());
      }
    }
    let _ = stdout.flush();
  } else {
    // stdout is a pipe or file carrying the document text; keep the notice
    // out of it. On these non-TUI paths stderr is still the user's terminal.
    let _ = writeln!(std::io::stderr(), "{summary}");
  }
}

fn write_crash_file(
  dir: &Path,
  name: &str,
  report: &str,
) -> std::io::Result<PathBuf> {
  std::fs::create_dir_all(dir)?;
  let path = dir.join(name);
  std::fs::write(&path, report)?;
  Ok(path)
}

/// Best-effort undo of what the reader and home picker set up, ignoring every
/// error. The alternate screen is only left while raw mode is still on (i.e.
/// the TUI was actually up): leaving it when it was never entered restores a
/// stale saved cursor position on xterm-likes and scribbles over the shell.
fn restore_terminal() {
  let mut stdout = std::io::stdout();
  if !stdout.is_terminal() {
    return;
  }
  if terminal::is_raw_mode_enabled().unwrap_or(false) {
    let _ = terminal::disable_raw_mode();
    let _ = execute!(stdout, terminal::LeaveAlternateScreen);
  }
  let _ =
    execute!(stdout, cursor::Show, cursor::SetCursorStyle::DefaultUserShape);
}
