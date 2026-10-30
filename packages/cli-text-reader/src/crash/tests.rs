use super::report::*;
use super::{CrashGuard, RECORDS, install_panic_hook};
use chrono::TimeZone;
use std::backtrace::Backtrace;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

fn record(message: &str, location: Option<&str>) -> PanicRecord {
  PanicRecord {
    thread_id: std::thread::current().id(),
    thread_name: "main".to_string(),
    message: message.to_string(),
    location: location.map(str::to_string),
    time: chrono::Utc.with_ymd_and_hms(2026, 10, 29, 8, 5, 9).unwrap(),
    backtrace: Backtrace::disabled(),
  }
}

#[test]
fn payload_message_handles_str_string_and_other() {
  let s: Box<dyn std::any::Any + Send> = Box::new("literal");
  assert_eq!(payload_message(s.as_ref()), "literal");
  let s: Box<dyn std::any::Any + Send> = Box::new(String::from("formatted"));
  assert_eq!(payload_message(s.as_ref()), "formatted");
  let s: Box<dyn std::any::Any + Send> = Box::new(42u32);
  assert_eq!(payload_message(s.as_ref()), "Box<dyn Any>");
}

#[test]
fn report_has_version_thread_location_message_and_backtrace() {
  let fatal = record("byte index 3 is not a char boundary", Some("a.rs:7:9"));
  let report = format_report(&fatal, &[], "9.9.9");
  assert!(report.starts_with("hygg crash report\n"));
  assert!(report.contains("Version:  hygg 9.9.9\n"));
  assert!(report.contains("Time:     2026-10-29T08:05:09+00:00\n"));
  assert!(report.contains("Thread:   main\n"));
  assert!(report.contains("Location: a.rs:7:9\n"));
  assert!(report.contains("Message:  byte index 3 is not a char boundary\n"));
  assert!(report.contains("Backtrace:\n"));
  assert!(report.contains(ISSUES_URL));
  assert!(!report.contains("Earlier panics"));
}

#[test]
fn report_lists_earlier_panics_after_the_fatal_one() {
  let fatal = record("fatal", Some("main.rs:1:1"));
  let earlier = record("worker died", Some("worker.rs:2:2"));
  let report = format_report(&fatal, &[&earlier], "1.0.0");
  let fatal_at = report.find("Message:  fatal").unwrap();
  let section_at = report.find("Earlier panics in this session").unwrap();
  let earlier_at = report.find("Message:  worker died").unwrap();
  assert!(fatal_at < section_at && section_at < earlier_at);
}

#[test]
fn user_message_points_at_the_crash_file() {
  let fatal = record("boom", Some("src/x.rs:10:5"));
  let msg = user_message(&fatal, Ok(Path::new("/c/crash.log")));
  assert_eq!(
    msg,
    format!(
      "hygg crashed: boom at src/x.rs:10:5. Details: /c/crash.log. \
       Please report at {ISSUES_URL}"
    )
  );
  let msg = user_message(&record("boom", None), Err("read-only"));
  assert!(msg.contains("boom at <unknown location>"));
  assert!(msg.contains("No crash file could be written (read-only)"));
}

#[test]
fn log_dir_prefers_override_then_config_then_temp() {
  let over = PathBuf::from("/over");
  let cfg = PathBuf::from("/cfg");
  assert_eq!(log_dir(Some(over.clone()), Some(cfg.clone())), over);
  assert_eq!(log_dir(None, Some(cfg)), PathBuf::from("/cfg/hygg/logs"));
  assert_eq!(log_dir(None, None), fallback_log_dir());
  assert!(fallback_log_dir().starts_with(std::env::temp_dir()));
}

#[test]
fn crash_file_name_is_sortable_and_unique_per_process() {
  let t = chrono::Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap();
  assert_eq!(crash_file_name(t, 4242), "crash-20260102T030405Z-4242.log");
}

/// The panic hook and `RECORDS` are process-global; serialize the tests that
/// install the real hook so they don't see each other's panics.
static HOOK_LOCK: Mutex<()> = Mutex::new(());

/// Installs the crash hook for the duration of `f`, then puts back whatever
/// hook was there before so the rest of the test binary is unaffected.
fn with_crash_hook(f: impl FnOnce()) {
  let _lock = HOOK_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
  let previous = std::panic::take_hook();
  // `take_hook` left std's default in place, so ours chains to that.
  drop(install_panic_hook());
  f();
  let _ = std::panic::take_hook();
  std::panic::set_hook(previous);
  RECORDS.lock().unwrap_or_else(PoisonError::into_inner).clear();
}

fn crash_files(dir: &Path) -> Vec<PathBuf> {
  std::fs::read_dir(dir)
    .map(|it| it.filter_map(|e| e.ok().map(|e| e.path())).collect())
    .unwrap_or_default()
}

#[test]
fn fatal_panic_writes_a_crash_file_with_a_backtrace() {
  let tmp = tempfile::tempdir().unwrap();
  let dir = tmp.path().join("logs");
  with_crash_hook(|| {
    // A panic caught below the guard is recovered: recorded, but no file.
    let caught = std::panic::catch_unwind(|| panic!("recovered first"));
    assert!(caught.is_err());
    assert!(crash_files(&dir).is_empty());

    let result = std::panic::catch_unwind(|| {
      let _guard = CrashGuard::with_dir(dir.clone());
      panic!("boom {}", 7);
    });
    assert!(result.is_err());
  });

  let files = crash_files(&dir);
  assert_eq!(files.len(), 1, "{files:?}");
  let name = files[0].file_name().unwrap().to_string_lossy().into_owned();
  assert!(name.starts_with("crash-") && name.ends_with(".log"), "{name}");
  let report = std::fs::read_to_string(&files[0]).unwrap();
  assert!(report.contains("Message:  boom 7\n"), "{report}");
  assert!(report.contains("crash/tests.rs:"), "{report}");
  assert!(report.contains(env!("CARGO_PKG_VERSION")));
  // A real, resolved backtrace naming this test, not a "disabled" stub.
  assert!(
    report.contains("fatal_panic_writes_a_crash_file_with_a_backtrace"),
    "{report}"
  );
  // The recovered panic is kept as context for the fatal one.
  assert!(report.contains("Message:  recovered first\n"), "{report}");
}

#[test]
fn guard_dropped_normally_writes_nothing() {
  let tmp = tempfile::tempdir().unwrap();
  with_crash_hook(|| {
    drop(CrashGuard::with_dir(tmp.path().to_path_buf()));
  });
  assert!(crash_files(tmp.path()).is_empty());
}
