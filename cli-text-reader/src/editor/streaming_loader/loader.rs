use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::thread::{self, JoinHandle};

use cli_pdf_to_text::{PdfRenderedPage, SharedPdfStream};

use super::super::streaming::PageLoaded;

/// Cap the channel so the worker can't run unboundedly far ahead of the
/// main thread.
pub(crate) const CHANNEL_BUFFER: usize = 32;

/// Block step used by the loader pattern: forward `BLOCK`, then backward
/// `BLOCK`, then forward `BLOCK`, etc.
const BLOCK: usize = 10;

/// Spawn the background page extraction thread.
///
/// The worker walks pages outwards from `start_page` in the documented
/// pattern: 10 forward, 10 backward, 10 forward, ... clamped at edges.
/// Pages listed in `already_loaded` (1-based) are skipped because they were
/// extracted synchronously during the open phase. If the starting page was
/// not preloaded, the worker renders it first so the viewport fills as soon
/// as possible.
///
/// `start_page` is 1-based.
///
/// Drops out when either the channel closes (consumer hung up) or the
/// shared `cancel` flag flips to `true`.
pub fn spawn_loader(
  stream: SharedPdfStream,
  start_page: usize,
  col: usize,
  already_loaded: Vec<usize>,
  cancel: Arc<AtomicBool>,
) -> (Receiver<PageLoaded>, JoinHandle<()>) {
  let total_pages = stream.total_pages();
  let (tx, rx) = mpsc::sync_channel::<PageLoaded>(CHANNEL_BUFFER);

  let handle = thread::Builder::new()
    .name("hygg-pdf-loader".into())
    .spawn(move || {
      run_loader(
        stream,
        start_page,
        col,
        already_loaded,
        total_pages,
        tx.clone(),
        Arc::clone(&cancel),
      );
    })
    .expect("spawning pdf loader thread");

  (rx, handle)
}

fn run_loader(
  stream: SharedPdfStream,
  start_page: usize,
  col: usize,
  already_loaded: Vec<usize>,
  total_pages: usize,
  tx: SyncSender<PageLoaded>,
  cancel: Arc<AtomicBool>,
) {
  if total_pages == 0 {
    return;
  }
  let start = start_page.clamp(1, total_pages);
  let skip: std::collections::HashSet<usize> =
    already_loaded.into_iter().collect();

  for page_1based in loader_order(start, total_pages, &skip) {
    if cancel.load(Ordering::Relaxed) {
      break;
    }
    let rendered_page = render_or_blank(&stream, page_1based, col);
    let message = PageLoaded::Page {
      page_index: page_1based - 1,
      rendered_page,
      replace_existing: false,
    };

    send_page_loaded(&tx, message, cancel.as_ref());
  }
}

pub(crate) fn render_or_blank(
  stream: &cli_pdf_to_text::PdfStream,
  page_1based: usize,
  col: usize,
) -> PdfRenderedPage {
  stream.extract_page_with_images(page_1based, col).unwrap_or_else(|| {
    PdfRenderedPage {
      raw_text: String::new(),
      lines: vec![String::new()],
      line_kinds: vec![cli_pdf_to_text::PdfLineKind::Text],
      contains_images: false,
    }
  })
}

pub(crate) fn send_page_loaded(
  tx: &SyncSender<PageLoaded>,
  message: PageLoaded,
  cancel: &AtomicBool,
) {
  let mut payload = Some(message);
  while let Some(msg) = payload.take() {
    if cancel.load(Ordering::Relaxed) {
      return;
    }
    match tx.try_send(msg) {
      Ok(()) => break,
      Err(TrySendError::Full(returned)) => {
        payload = Some(returned);
        thread::sleep(std::time::Duration::from_millis(20));
      }
      Err(TrySendError::Disconnected(_)) => return,
    }
  }
}

/// Build the page-load order starting from `start` (1-based). The starting
/// page itself is skipped — it's loaded synchronously up front. The
/// remaining pages are visited in alternating forward / backward blocks of
/// `BLOCK` pages each, clamped at document edges.
pub fn load_order(start: usize, total: usize) -> Vec<usize> {
  if total == 0 {
    return Vec::new();
  }
  let start = start.clamp(1, total);

  let mut forward_next = start + 1; // next forward page to emit
  let mut backward_next = start; // next backward page to emit (subtract 1)
  let mut order = Vec::with_capacity(total.saturating_sub(1));
  let mut forward_turn = true;

  while order.len() + 1 < total {
    let progressed_this_block = if forward_turn {
      let mut emitted = 0usize;
      while emitted < BLOCK && forward_next <= total {
        order.push(forward_next);
        forward_next += 1;
        emitted += 1;
      }
      emitted
    } else {
      let mut emitted = 0usize;
      while emitted < BLOCK && backward_next > 1 {
        backward_next -= 1;
        order.push(backward_next);
        emitted += 1;
      }
      emitted
    };

    forward_turn = !forward_turn;

    if progressed_this_block == 0 {
      // One side is exhausted; the other side may still have work, so
      // just continue the loop — it'll fall through naturally until both
      // exhaust.
      if forward_next > total && backward_next <= 1 {
        break;
      }
    }
  }

  order
}

pub(crate) fn loader_order(
  start: usize,
  total: usize,
  skip: &std::collections::HashSet<usize>,
) -> Vec<usize> {
  if total == 0 {
    return Vec::new();
  }
  let start = start.clamp(1, total);
  let mut order = Vec::with_capacity(total.saturating_sub(skip.len()));
  if !skip.contains(&start) {
    order.push(start);
  }
  order.extend(
    load_order(start, total).into_iter().filter(|page| !skip.contains(page)),
  );
  order
}
