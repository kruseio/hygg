use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::debug;
use crate::editor::Editor;
use crate::editor::streaming::{PendingPdfStream, StreamReady};
use crate::editor::streaming_loader::spawn_loader;
use crate::progress::load_progress;

const PDF_PRELOAD_RADIUS: usize = 10;

#[derive(Clone, Copy, Debug, Default)]
struct SavedPdfPosition {
  target_page: Option<usize>,
  line_in_page: Option<usize>,
  cursor_y: Option<usize>,
  flat_offset: Option<usize>,
}

impl SavedPdfPosition {
  fn from_progress(progress: crate::progress::Progress) -> Self {
    let target_page = progress.page.map(|page| page as usize);
    Self {
      target_page,
      line_in_page: progress.line_in_page,
      cursor_y: progress.cursor_y,
      flat_offset: target_page.is_none().then_some(progress.offset),
    }
  }
}

fn load_saved_pdf_position(document_hash: u64) -> SavedPdfPosition {
  load_progress(document_hash)
    .map(SavedPdfPosition::from_progress)
    .unwrap_or_default()
}

pub(crate) fn infer_pdf_position_from_flat_offset(
  stream: &cli_pdf_to_text::PdfStream,
  flat_offset: usize,
  col: usize,
) -> Option<(usize, usize)> {
  let total_pages = stream.total_pages();
  if total_pages == 0 {
    return None;
  }

  let mut remaining = flat_offset;
  for page in 1..=total_pages {
    let page_lines = stream
      .extract_page(page)
      .map(|raw| {
        crate::editor::streaming::LoadedPage::from_raw(raw, col)
          .standalone_lines
          .len()
          .max(1)
      })
      .unwrap_or(1);

    if remaining < page_lines {
      return Some((page, remaining));
    }
    remaining = remaining.saturating_sub(page_lines);

    if page < total_pages {
      if remaining == 0 {
        return Some((page + 1, 0));
      }
      remaining = remaining.saturating_sub(1);
    }
  }

  Some((total_pages, 0))
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

  // Spawn the open in the background so the editor can paint immediately.
  let (ready_tx, ready_rx) = std::sync::mpsc::channel::<StreamReady>();
  let path_for_thread = canonical_str.clone();
  let saved_position = load_saved_pdf_position(document_hash);
  let saved_target_page = saved_position.target_page.unwrap_or(1);
  let saved_line_in_page = saved_position.line_in_page;
  let saved_cursor_y = saved_position.cursor_y;

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
            let (target_page, restore_line_in_page) = if let Some(target_page) =
              saved_position.target_page
            {
              (target_page.clamp(1, total_pages), saved_position.line_in_page)
            } else if let Some(flat_offset) = saved_position.flat_offset {
              match infer_pdf_position_from_flat_offset(
                &stream,
                flat_offset,
                col,
              ) {
                Some((page, line)) => (page.clamp(1, total_pages), Some(line)),
                None => (1, None),
              }
            } else {
              (1, None)
            };
            let preloaded_pages: Vec<_> = if bundled_ocr {
              Vec::new()
            } else {
              let lo = target_page.saturating_sub(preload_radius).max(1);
              let hi = (target_page + preload_radius).min(total_pages);
              (lo..=hi)
                .filter_map(|p| {
                  stream.extract_page_with_images(p, col).map(|page| (p, page))
                })
                .collect()
            };
            let preloaded_indices: Vec<usize> =
              preloaded_pages.iter().map(|(p, _)| *p).collect();
            let shared = Arc::new(stream);
            let cancel = Arc::new(AtomicBool::new(false));
            let (pages_rx, worker) = spawn_loader(
              Arc::clone(&shared),
              target_page,
              col,
              preloaded_indices,
              Arc::clone(&cancel),
            );
            StreamReady::Ok {
              stream: shared,
              target_page,
              restore_line_in_page,
              preloaded_pages,
              pages_receiver: pages_rx,
              cancel,
              worker,
              ocr_loading: false,
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
  editor.pdf_source_path = Some(canonical_str.clone());
  editor.ocr_enabled = bundled_ocr;
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
    if let Some(cancel) = state.ocr_cancel.take() {
      cancel.store(true, Ordering::Relaxed);
    }
    if let Some(handle) = state.worker.take() {
      let _ = handle.join();
    }
    if let Some(handle) = state.ocr_worker.take() {
      let _ = handle.join();
    }
  }

  debug::debug_log("main", "Streaming editor run completed");
  debug::flush_debug_log();
  result
}

pub(crate) fn pdf_preload_radius(bundled_ocr: bool) -> usize {
  if bundled_ocr { 0 } else { PDF_PRELOAD_RADIUS }
}
