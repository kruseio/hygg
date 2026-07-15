//! Throttled progress save and the best-effort push to the server.

use leptos::prelude::*;
use leptos::task::spawn_local;

use super::{Creds, push_creds};
use crate::app::SettingsCtx;
use crate::model::Book;
use crate::{layout, storage, sync};

/// Cap on the reading time counted for a single gap between saves, so leaving
/// the reader open and idle doesn't inflate the total.
const IDLE_CAP_SECS: f64 = 60.0;

#[allow(clippy::too_many_arguments)]
pub fn save_progress_throttled(
  id: String,
  book: RwSignal<Option<Book>>,
  // The document line at the viewport center (the synced/restored anchor).
  line: usize,
  last_save: RwSignal<f64>,
  creds: Option<Creds>,
  scope: hygg_shared::sync::AutoSyncPolicy,
) {
  let now = js_sys::Date::now();
  let prev = last_save.get_untracked();
  if now - prev < 700.0 {
    return;
  }
  last_save.set(now);
  let (total, page_anchor, word_offset, percent) = book.with_untracked(|b| {
    b.as_ref().map_or((0u64, None, None, 0.0), |x| {
      (
        x.lines.len() as u64,
        x.page_of_line(line),
        Some(x.word_offset_of_line(line) as u64),
        x.percent_of_line(line),
      )
    })
  });
  // Count active reading since the previous save (capped); zero on the first
  // save, which has no prior timestamp to measure against.
  let add_seconds =
    if prev > 0.0 { ((now - prev) / 1000.0).min(IDLE_CAP_SECS) } else { 0.0 };
  spawn_local(persist_progress(
    id,
    line,
    percent,
    total,
    page_anchor,
    word_offset,
    add_seconds,
    creds,
    scope,
  ));
}

/// Persist the final position when leaving the reader: recompute the center
/// line from the current scroll/viewport and save it (a clean exit must never
/// push 0% and wipe a position synced from another device). Factored out of
/// `reader.rs`'s `on_cleanup` to keep that file within the LOC budget.
pub fn persist_on_exit(
  id: String,
  book: RwSignal<Option<Book>>,
  scroll_top: RwSignal<f64>,
  viewport_h: RwSignal<f64>,
  viewport_w: RwSignal<f64>,
  settings: SettingsCtx,
) {
  let col = book.with_untracked(|b| b.as_ref().map_or(64, |x| x.col.max(1)));
  let lh = layout::fit_font_px(
    viewport_w.get_untracked().max(1.0),
    col,
    settings.with_untracked(|s| s.text_zoom) as f64,
  )
  .round()
  .max(1.0);
  let st = scroll_top.get_untracked();
  let vh = viewport_h.get_untracked().max(1.0);
  let line = ((st + vh / 2.0) / lh).floor().max(0.0) as usize;
  let (total, page_anchor, word_offset, percent) = book.with_untracked(|b| {
    b.as_ref().map_or((0u64, None, None, 0.0), |x| {
      (
        x.lines.len() as u64,
        x.page_of_line(line),
        Some(x.word_offset_of_line(line) as u64),
        x.percent_of_line(line),
      )
    })
  });
  let creds = settings.with_untracked(push_creds);
  let scope = settings.with_untracked(|s| s.auto_sync_scope);
  spawn_local(persist_progress(
    id,
    line,
    percent,
    total,
    page_anchor,
    word_offset,
    0.0,
    creds,
    scope,
  ));
}

/// Merge a new position into stored progress, preserving accumulated reading
/// seconds and adding `add_seconds`, bumping the last-read timestamp, and
/// best-effort pushing the position to the server.
#[allow(clippy::too_many_arguments)]
pub async fn persist_progress(
  id: String,
  line: usize,
  percent: f64,
  total: u64,
  page_anchor: Option<(u32, usize)>,
  word_offset: Option<u64>,
  add_seconds: f64,
  creds: Option<Creds>,
  // Automatic-sync scope: gates whether this document pushes at all (combined
  // with its opt-in and the book heuristic).
  scope: hygg_shared::sync::AutoSyncPolicy,
) {
  let mut p = storage::get_progress(&id).await.unwrap_or_default();
  p.line = line;
  p.percent = percent;
  p.seconds += add_seconds;
  // Stamp the last-write-wins baseline in the server's clock domain, matching
  // the pushed op's timestamp so it compares correctly against peers.
  p.updated_at = crate::clock::now_ms();
  let _ = storage::put_progress(&id, p).await;
  if let Some((creds, device)) = creds {
    // Push only when the effective `SyncMode` permits state *and* the scope
    // covers this document (`off` keeps it local; a report the scope doesn't
    // cover stays on this device).
    let syncs = storage::get_summary(&id)
      .await
      .map(|s| s.effective_sync_mode().syncs_state() && s.auto_syncs(scope))
      .unwrap_or(false);
    if !syncs {
      return;
    }
    let (page, line_in_page) = match page_anchor {
      Some((pg, lip)) => (Some(pg), Some(lip as u64)),
      None => (None, None),
    };
    let _ = sync::push_progress(
      &creds,
      &device,
      &id,
      line as u64,
      total,
      p.percent,
      page,
      line_in_page,
      word_offset,
    )
    .await;
  }
}
