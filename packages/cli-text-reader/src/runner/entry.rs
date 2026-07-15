use crate::debug;
use crate::editor::{Editor, RunOutcome};
use crate::library::{LibraryEntry, record_open};

pub fn run_cli_text_reader(
  lines: Vec<String>,
  col: usize,
) -> Result<RunOutcome, Box<dyn std::error::Error>> {
  run_cli_text_reader_with_demo(lines, col, false)
}

pub fn run_cli_text_reader_with_demo(
  lines: Vec<String>,
  col: usize,
  demo_mode: bool,
) -> Result<RunOutcome, Box<dyn std::error::Error>> {
  run_cli_text_reader_inner(lines, col, None, demo_mode, None)
}

pub fn run_cli_text_reader_with_content(
  lines: Vec<String>,
  col: usize,
  raw_content: Option<String>,
  demo_mode: bool,
) -> Result<RunOutcome, Box<dyn std::error::Error>> {
  run_cli_text_reader_inner(lines, col, raw_content, demo_mode, None)
}

/// Like `run_cli_text_reader_with_content`, but records the document in the
/// local library index (keyed by `source_path`) so it appears on `:home` and
/// can be re-opened to resume. Used for real file opens.
pub fn run_cli_text_reader_with_source(
  lines: Vec<String>,
  col: usize,
  raw_content: Option<String>,
  source_path: Option<String>,
) -> Result<RunOutcome, Box<dyn std::error::Error>> {
  run_cli_text_reader_inner(lines, col, raw_content, false, source_path)
}

fn run_cli_text_reader_inner(
  lines: Vec<String>,
  col: usize,
  raw_content: Option<String>,
  demo_mode: bool,
  source_path: Option<String>,
) -> Result<RunOutcome, Box<dyn std::error::Error>> {
  // Initialize debug logging
  debug::init_debug_logging()?;
  debug::debug_log("main", "Starting cli-text-reader");
  debug::debug_log_state("main", "lines_count", &lines.len().to_string());
  debug::debug_log_state("main", "col", &col.to_string());
  debug::debug_log_state("main", "demo_mode", &demo_mode.to_string());
  if raw_content.is_some() {
    debug::debug_log("main", "Raw content provided for consistent hashing");
  }

  let total_lines = lines.len();
  let book_id =
    raw_content.as_deref().map(hygg_shared::sync::book_id_from_text);
  let mut editor = if let Some(content) = raw_content {
    Editor::new_with_content(lines, col, content)
  } else {
    Editor::new(lines, col)
  };
  editor.book_id = book_id.clone();
  editor.tutorial_demo_mode = demo_mode;

  if let Some(path) = source_path.as_deref() {
    editor.source_path = Some(path.to_string());
    let mut entry =
      LibraryEntry::from_path(editor.document_hash, book_id, path, total_lines);
    // Carry the per-document sync preference across re-opens (a fresh
    // `from_path` would otherwise reset it), then seed the reader's effective
    // mode so it gates sync from the first scroll.
    if let Some(prev) = crate::library::latest_entry(editor.document_hash) {
      entry.local_sync_mode = prev.local_sync_mode;
      entry.server_sync_mode = prev.server_sync_mode;
      entry.auto_sync_optin = prev.auto_sync_optin;
    }
    editor.sync_mode = entry.effective_sync_mode();
    if let Err(e) = record_open(&entry) {
      debug::debug_log_error("library", &format!("record_open failed: {e}"));
    }
  }

  let result = editor.run();

  debug::debug_log("main", "Editor run completed");
  debug::flush_debug_log();
  result
}

pub fn run_cli_text_reader_with_demo_id(
  lines: Vec<String>,
  col: usize,
  demo_id: usize,
) -> Result<RunOutcome, Box<dyn std::error::Error>> {
  // Initialize debug logging
  debug::init_debug_logging()?;
  debug::debug_log("main", "Starting cli-text-reader with demo");
  debug::debug_log_state("main", "demo_id", &demo_id.to_string());
  debug::debug_log_state("main", "col", &col.to_string());

  let mut editor = Editor::new(lines, col);
  editor.tutorial_demo_mode = true;
  editor.demo_id = Some(demo_id);
  let result = editor.run();

  debug::debug_log("main", "Editor run completed");
  debug::flush_debug_log();
  result
}
