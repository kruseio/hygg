mod bookmarks;
mod config;
mod core_state;
mod core_types;
mod debug;
pub mod demo_components;
mod demo_content;
pub mod demo_registry;
pub mod demo_script;
mod demo_tutorial_test;
mod editor;
mod help;
mod highlights;
mod highlights_core;
mod highlights_persistence;
mod interactive_tutorial;
mod interactive_tutorial_buffer;
mod interactive_tutorial_steps;
mod interactive_tutorial_tests;
mod interactive_tutorial_utils;
mod progress;
mod tutorial;
mod utils;

use editor::Editor;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::editor::streaming::{PendingPdfStream, StreamReady};
use crate::editor::streaming_loader::spawn_loader;
use crate::progress::load_progress;

const PDF_PRELOAD_RADIUS: usize = 10;

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

pub fn run_cli_text_reader_pdf_path(
  pdf_path: String,
  col: usize,
) -> Result<(), Box<dyn std::error::Error>> {
  run_cli_text_reader_pdf_path_inner(pdf_path, col, false)
}

pub fn run_cli_text_reader_pdf_path_with_bundled_ocr(
  pdf_path: String,
  col: usize,
) -> Result<(), Box<dyn std::error::Error>> {
  run_cli_text_reader_pdf_path_inner(pdf_path, col, true)
}

fn run_cli_text_reader_pdf_path_inner(
  pdf_path: String,
  col: usize,
  bundled_ocr: bool,
) -> Result<(), Box<dyn std::error::Error>> {
  debug::init_debug_logging()?;
  debug::debug_log(
    "main",
    "Starting cli-text-reader (PDF streaming, deferred open)",
  );
  debug::debug_log_state("main", "col", &col.to_string());

  // Canonicalize the path up front so the document hash is stable across
  // sessions even when the user passes different relative paths.
  let canonical_path = hygg_shared::normalize_file_path(&pdf_path)?;
  let canonical_str = canonical_path.to_string_lossy().to_string();
  let document_hash = crate::progress::generate_hash(&canonical_str);

  // Spawn the open + target-page extract in the background so the editor
  // can paint a "Loading…" splash immediately.
  let (ready_tx, ready_rx) = std::sync::mpsc::channel::<StreamReady>();
  let path_for_thread = canonical_str.clone();
  let (saved_target_page, saved_line_in_page, saved_cursor_y) =
    match load_progress(document_hash) {
      Ok(p) => {
        (p.page.map(|n| n as usize).unwrap_or(1), p.line_in_page, p.cursor_y)
      }
      Err(_) => (1, None, None),
    };

  // Size of the synchronous preload window around the cursor's saved page.
  // Picked so the viewport is fully covered by real content on first render
  // even for dense PDFs, and so seam stitching between the page and its
  // immediate neighbours is stable from frame one (eliminates the
  // placeholder->loaded flicker the user sees while pages stream in).
  let preload_radius = pdf_preload_radius(bundled_ocr);

  std::thread::Builder::new().name("hygg-pdf-opener".into()).spawn(
    move || {
      let opened = cli_pdf_to_text::PdfStream::open(&path_for_thread);
      let message = match opened {
        Ok(stream) => {
          let total_pages = stream.total_pages();
          if total_pages == 0 {
            StreamReady::Err("PDF parsed but reports zero pages".to_string())
          } else {
            let target_page = saved_target_page.clamp(1, total_pages);
            let lo = target_page.saturating_sub(preload_radius).max(1);
            let hi = (target_page + preload_radius).min(total_pages);
            let preloaded_pages: Vec<_> = (lo..=hi)
              .filter_map(|p| {
                stream.extract_page_with_images(p, col).map(|page| (p, page))
              })
              .collect();
            let preloaded_indices: Vec<usize> =
              preloaded_pages.iter().map(|(p, _)| *p).collect();
            let shared = Arc::new(stream);
            let cancel = Arc::new(AtomicBool::new(false));
            let ocr_pdf_path = bundled_ocr.then(|| path_for_thread.clone());
            let (pages_rx, worker) = spawn_loader(
              Arc::clone(&shared),
              ocr_pdf_path,
              target_page,
              col,
              preloaded_indices,
              Arc::clone(&cancel),
            );
            StreamReady::Ok {
              stream: shared,
              target_page,
              preloaded_pages,
              pages_receiver: pages_rx,
              cancel,
              worker,
              ocr_loading: bundled_ocr,
            }
          }
        }
        Err(e) => StreamReady::Err(format!("Failed to open PDF: {e}")),
      };
      let _ = ready_tx.send(message);
    },
  )?;

  // Splash buffer is intentionally blank — the cursor and highlight bar
  // render during the splash too, but at the *predicted* row the install
  // path will land on. That way the cursor and highlight bar don't appear
  // to hop when the streaming state takes over: they're already in the
  // final position, and only the surrounding lines fill in.
  let mut editor =
    Editor::new_with_content(vec![String::new()], col, canonical_str.clone());
  editor.document_hash = document_hash;
  editor.pdf_pending = Some(PendingPdfStream {
    receiver: ready_rx,
    started_at: std::time::Instant::now(),
    canonical_path_display: canonical_str,
    restore_line_in_page: saved_line_in_page,
    restore_cursor_y: saved_cursor_y,
  });

  // Mirror the cursor placement that `poll_pending_pdf_stream` will do once
  // the open completes:
  //   - target_page == 1 and line_in_page < center_y → cursor_y = line_in_page
  //   - otherwise                                    → cursor_y = center_y
  // For target_page > 1 the preload window guarantees that
  // target_line_start ≥ center_y in practice, so center_y is the right
  // landing row.
  let content_height = editor.height.saturating_sub(1);
  let center_y = content_height / 2;
  let line_in_page_hint = saved_line_in_page.unwrap_or(0);
  let restore_cursor_y = saved_cursor_y.unwrap_or(center_y);
  let predicted_cursor_y =
    if saved_target_page == 1 && line_in_page_hint < restore_cursor_y {
      line_in_page_hint
    } else {
      restore_cursor_y.min(content_height.saturating_sub(1))
    };
  editor.cursor_y = predicted_cursor_y;

  let result = editor.run();

  // Stop any loader thread and join it before we return.
  if let Some(state) = editor.pdf_streaming.as_mut() {
    state.cancel.store(true, Ordering::Relaxed);
    if let Some(handle) = state.worker.take() {
      let _ = handle.join();
    }
  }

  debug::debug_log("main", "Streaming editor run completed");
  debug::flush_debug_log();
  result
}

fn pdf_preload_radius(bundled_ocr: bool) -> usize {
  if bundled_ocr { 0 } else { PDF_PRELOAD_RADIUS }
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn ocr_pdf_streams_with_smaller_initial_preload() {
    assert_eq!(pdf_preload_radius(false), 10);
    assert_eq!(pdf_preload_radius(true), 0);
  }
}
