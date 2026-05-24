use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::thread::{self, JoinHandle};

use cli_pdf_to_text::SharedPdfStream;

use super::streaming::PageLoaded;

/// Cap the channel so the worker can't run unboundedly far ahead of the
/// main thread.
const CHANNEL_BUFFER: usize = 32;

/// Block step used by the loader pattern: forward `BLOCK`, then backward
/// `BLOCK`, then forward `BLOCK`, etc.
const BLOCK: usize = 10;

/// Spawn the background page extraction thread.
///
/// The worker walks pages outwards from `start_page` in the documented
/// pattern: 10 forward, 10 backward, 10 forward, ... clamped at edges.
/// Pages listed in `already_loaded` (1-based) are skipped because they were
/// extracted synchronously during the open phase.
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
        tx,
        cancel,
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

  for page_1based in load_order(start, total_pages) {
    if cancel.load(Ordering::Relaxed) {
      break;
    }
    if skip.contains(&page_1based) {
      continue;
    }
    let rendered_page = stream
      .extract_page_with_images(page_1based, col)
      .unwrap_or_else(|| cli_pdf_to_text::PdfRenderedPage {
        raw_text: String::new(),
        lines: vec![String::new()],
        line_kinds: vec![cli_pdf_to_text::PdfLineKind::Text],
        contains_images: false,
      });
    let message = PageLoaded { page_index: page_1based - 1, rendered_page };

    // Use a small spin so we honour cancellation while the channel is full.
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn pattern_emits_forward_then_backward_blocks() {
    let order = load_order(50, 200);
    // First block: forward 10 starting at 51
    assert_eq!(&order[..10], &[51, 52, 53, 54, 55, 56, 57, 58, 59, 60]);
    // Second block: backward 10 from 49
    assert_eq!(&order[10..20], &[49, 48, 47, 46, 45, 44, 43, 42, 41, 40]);
    // Third block: forward 10 from 61
    assert_eq!(&order[20..30], &[61, 62, 63, 64, 65, 66, 67, 68, 69, 70]);
    // Fourth block: backward 10 from 39
    assert_eq!(&order[30..40], &[39, 38, 37, 36, 35, 34, 33, 32, 31, 30]);
  }

  #[test]
  fn pattern_handles_edge_when_start_is_at_top() {
    let order = load_order(1, 25);
    // Backward exhausts immediately, so the sequence is pure forward.
    assert_eq!(order, (2..=25).collect::<Vec<_>>());
  }

  #[test]
  fn pattern_handles_edge_when_start_is_near_bottom() {
    let order = load_order(95, 100);
    // First block forward: 96..=100 (only 5 available)
    assert_eq!(&order[..5], &[96, 97, 98, 99, 100]);
    // Then backward block of 10 from 94
    assert_eq!(&order[5..15], &[94, 93, 92, 91, 90, 89, 88, 87, 86, 85]);
  }

  #[test]
  fn pattern_visits_every_page_exactly_once() {
    for total in [1, 2, 5, 11, 21, 50, 199] {
      for start in [1, total / 2, total] {
        if start == 0 || start > total {
          continue;
        }
        let order = load_order(start, total);
        let mut seen: Vec<usize> = order.clone();
        seen.sort_unstable();
        let mut expected: Vec<usize> = (1..=total).collect();
        expected.retain(|p| *p != start);
        assert_eq!(seen, expected, "total={total} start={start}");
      }
    }
  }
}
