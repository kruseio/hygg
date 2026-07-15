use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

static DEBUG_FILE: Mutex<Option<std::fs::File>> = Mutex::new(None);

pub fn init_debug_logging() -> Result<(), Box<dyn std::error::Error>> {
  eprintln!("DEBUG: init_debug_logging called");
  if std::env::var("HYGG_DEBUG").is_ok() {
    eprintln!("DEBUG: HYGG_DEBUG is set");
    let mut debug_dir =
      dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    debug_dir.push(".hygg");
    eprintln!("DEBUG: Creating debug dir: {debug_dir:?}");
    std::fs::create_dir_all(&debug_dir)?;

    let debug_file_path = debug_dir.join("debug.log");
    eprintln!("DEBUG: Creating debug file: {debug_file_path:?}");

    let file = OpenOptions::new()
      .create(true)
      .truncate(true) // Clear previous log
      .write(true)
      .open(&debug_file_path)?;

    let mut debug_file = DEBUG_FILE
      .lock()
      .map_err(|e| format!("Failed to acquire debug file mutex: {e}"))?;
    *debug_file = Some(file);

    // Write initial log entry
    if let Some(ref mut file) = *debug_file {
      writeln!(file, "=== HYGG DEBUG LOG STARTED ===")?;
      writeln!(
        file,
        "Timestamp: {}",
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
      )?;
      writeln!(
        file,
        "Environment: HYGG_DEBUG={}",
        std::env::var("HYGG_DEBUG").unwrap_or_default()
      )?;
      writeln!(file, "===============================")?;
      file.flush()?;
      eprintln!("DEBUG: Initial log entries written");
    }
  } else {
    eprintln!("DEBUG: HYGG_DEBUG not set, skipping debug logging");
  }
  Ok(())
}

pub fn debug_log(module: &str, message: &str) {
  if std::env::var("HYGG_DEBUG").is_ok() {
    let timestamp = chrono::Utc::now().format("%H:%M:%S%.3f");
    let log_line = format!("[{timestamp}] [{module}] {message}");

    eprintln!("{log_line}");

    // Write to file
    if let Ok(mut debug_file) = DEBUG_FILE.lock()
      && let Some(ref mut file) = *debug_file
    {
      if let Err(e) = writeln!(file, "{log_line}") {
        eprintln!("Failed to write to debug log: {e}");
      } else {
        let _ = file.flush(); // Ensure immediate write
      }
    }
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
