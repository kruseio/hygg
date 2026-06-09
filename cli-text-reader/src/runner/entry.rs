use crate::debug;
use crate::editor::Editor;

pub fn run_cli_text_reader(
  lines: Vec<String>,
  col: usize,
) -> Result<(), Box<dyn std::error::Error>> {
  run_cli_text_reader_with_demo(lines, col, false)
}

pub fn run_cli_text_reader_with_demo(
  lines: Vec<String>,
  col: usize,
  demo_mode: bool,
) -> Result<(), Box<dyn std::error::Error>> {
  run_cli_text_reader_with_content(lines, col, None, demo_mode)
}

pub fn run_cli_text_reader_with_content(
  lines: Vec<String>,
  col: usize,
  raw_content: Option<String>,
  demo_mode: bool,
) -> Result<(), Box<dyn std::error::Error>> {
  // Initialize debug logging
  debug::init_debug_logging()?;
  debug::debug_log("main", "Starting cli-text-reader");
  debug::debug_log_state("main", "lines_count", &lines.len().to_string());
  debug::debug_log_state("main", "col", &col.to_string());
  debug::debug_log_state("main", "demo_mode", &demo_mode.to_string());
  if raw_content.is_some() {
    debug::debug_log("main", "Raw content provided for consistent hashing");
  }

  let mut editor = if let Some(content) = raw_content {
    Editor::new_with_content(lines, col, content)
  } else {
    Editor::new(lines, col)
  };
  editor.tutorial_demo_mode = demo_mode;
  let result = editor.run();

  debug::debug_log("main", "Editor run completed");
  debug::flush_debug_log();
  result
}

pub fn run_cli_text_reader_with_demo_id(
  lines: Vec<String>,
  col: usize,
  demo_id: usize,
) -> Result<(), Box<dyn std::error::Error>> {
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
