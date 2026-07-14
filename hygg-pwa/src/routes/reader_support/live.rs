//! Live position sync for the open document: subscribe to the server's SSE
//! `changed` stream and, when a peer advances *this* document past where we
//! are, raise a "jump to the new position?" toast. Tapping Jump moves there;
//! scrolling keeps our place (the throttled save pushes it, last-write-wins)
//! and dismisses the toast after a short grace — the same jump-or-keep-local UX
//! the CLI shows in its status line, so the two clients behave identically.

use gloo_timers::callback::Timeout;
use hygg_shared::sync::proto::ProgressDto;
use leptos::prelude::*;
use leptos::task::spawn_local;

use super::push_creds;
use crate::app::SettingsCtx;
use crate::model::Book;
use crate::sync;

/// How long the toast lingers after the reader starts scrolling before it
/// clears and the local position wins. Matches the CLI's grace so the UX is
/// identical across clients.
const DISMISS_GRACE_MS: u32 = 3_000;

/// The line in *this* book a server progress row points to, mapped
/// width-independently: the exact word anchor first, then the PDF page anchor
/// (scaled when the pagination differs), then the saved line or percentage. The
/// single mapping used both when opening a document and when a live `changed`
/// event advances it, so open-time resume and the jump toast always land on the
/// same line. Not clamped — callers clamp to their own line count.
pub fn server_line_for(book: &Book, row: &ProgressDto) -> usize {
  let total = book.lines.len();
  let by_word = match row.word_offset {
    Some(w) if w >= 0 => {
      Some(book.line_for_word(row.page.map(|pg| pg as u32), w as usize))
    }
    _ => None,
  };
  let by_page = match row.page {
    Some(pg) if pg > 0 => {
      let lip = row.line_in_page.unwrap_or(0).max(0) as usize;
      // A different width shifts everything within the (often figure-tall)
      // page, so scale the offset into this reader's own line space.
      let lip = if row.total_lines as usize == total || row.total_lines <= 0 {
        lip
      } else {
        (lip as f64 * total as f64 / row.total_lines as f64).round() as usize
      };
      book.line_of_page(pg as u32, lip)
    }
    _ => None,
  };
  by_word.or(by_page).unwrap_or_else(|| {
    if row.total_lines as usize == total && row.offset_line >= 0 {
      row.offset_line as usize
    } else if row.percentage > 0.0 && total > 0 {
      // The shared character fraction maps onto this reader's own lines.
      book.line_for_percent(row.percentage)
    } else {
      row.offset_line.max(0) as usize
    }
  })
}

/// The reader signals the live watcher and toast need. All `Copy` (signals and
/// node refs), so it threads cheaply through the closures below.
#[derive(Clone, Copy)]
pub struct LiveCtx {
  pub book: RwSignal<Option<Book>>,
  pub scroll_ref: NodeRef<leptos::html::Div>,
  pub scroll_top: RwSignal<f64>,
  pub viewport_h: RwSignal<f64>,
  pub viewport_w: RwSignal<f64>,
  pub settings: SettingsCtx,
  /// The target line of a pending server position (`Some` shows the toast).
  pub server_jump: RwSignal<Option<usize>>,
  /// Timestamp (epoch ms) of the position this device is showing, for the
  /// last-write-wins compare that decides whether the server is genuinely
  /// ahead.
  pub local_at: RwSignal<f64>,
  /// Whether a post-scroll dismiss is already scheduled (so it fires once).
  pub dismiss_scheduled: RwSignal<bool>,
  /// Set right before a programmatic scroll so its `scroll` event isn't
  /// treated as the user moving (shared with `reader.rs`'s `on_scroll`).
  pub suppress_next_scroll: RwSignal<bool>,
  /// Whether the user has genuinely moved since the position was adopted — a
  /// jump adopts the peer's place, so it clears this (see `jump_toast`).
  pub moved: RwSignal<bool>,
}

impl LiveCtx {
  /// This reader's line height in px (mirrors `reader.rs`'s `line_h`).
  fn line_h(&self) -> f64 {
    let col =
      self.book.with_untracked(|b| b.as_ref().map_or(64, |x| x.col.max(1)));
    let zoom = self.settings.with_untracked(|s| s.text_zoom) as f64;
    crate::layout::fit_font_px(
      self.viewport_w.get_untracked().max(1.0),
      col,
      zoom,
    )
    .round()
    .max(1.0)
  }

  /// The document line at the vertical center of the viewport right now.
  fn center_line(&self) -> usize {
    let lh = self.line_h();
    ((self.scroll_top.get_untracked() + self.viewport_h.get_untracked() / 2.0)
      / lh)
      .floor()
      .max(0.0) as usize
  }
}

/// Subscribe to the server's `changed` stream for the open document. On each
/// event we pull and, if this document's server position is newer than ours and
/// lands somewhere other than where we are (i.e. not just an echo of our own
/// push), raise the jump toast. Registers an `on_cleanup` that closes the
/// stream when the reader unmounts. No-op when not connected.
pub fn install(ctx: LiveCtx, book_id: String) {
  let Some((creds, _device)) = ctx.settings.with_untracked(push_creds) else {
    return;
  };
  let creds_cb = creds.clone();
  let events = crate::sse::connect(&creds, move || {
    let creds = creds_cb.clone();
    let id = book_id.clone();
    spawn_local(async move {
      let Ok(rows) = sync::pull_progress(&creds, None).await else {
        return;
      };
      let Some(row) = rows.into_iter().find(|p| p.book_id == id) else {
        return;
      };
      if (row.updated_at as f64) <= ctx.local_at.get_untracked() {
        return;
      }
      let Some(target) = ctx.book.with_untracked(|b| {
        b.as_ref().map(|book| {
          server_line_for(book, &row).min(book.lines.len().saturating_sub(1))
        })
      }) else {
        return;
      };
      // Ignore an echo landing where we already are; only a real move prompts.
      if target.abs_diff(ctx.center_line()) <= 1 {
        return;
      }
      // Raise (or refresh) the toast, invalidating any dismiss countdown still
      // queued for a prior one (see `note_scroll`).
      ctx.dismiss_scheduled.set(false);
      ctx.server_jump.set(Some(target));
    });
  });
  // Hold the subscription for the reader's lifetime; closing it on cleanup.
  // `SendWrapper` carries the non-`Send` `EventSource` across the cleanup bound
  // (sound: the app is single-threaded).
  let events = send_wrapper::SendWrapper::new(events);
  on_cleanup(move || drop(events));
}

/// Called on every scroll: the reader is at `local_at = now` (so a peer push
/// older than this won't prompt, and our own echo never does), and — while the
/// toast is up — scrolling starts the one-shot grace after which it clears and
/// the local position wins (the throttled save has already pushed it).
pub fn note_scroll(ctx: LiveCtx) {
  ctx.local_at.set(crate::clock::now_ms());
  if ctx.server_jump.get_untracked().is_none()
    || ctx.dismiss_scheduled.get_untracked()
  {
    return;
  }
  ctx.dismiss_scheduled.set(true);
  Timeout::new(DISMISS_GRACE_MS, move || {
    // Only clear if this countdown is still the live one — a jump, a manual
    // dismiss, or a newer toast resets the flag, invalidating a stale timer.
    if ctx.dismiss_scheduled.get_untracked() {
      ctx.server_jump.set(None);
      ctx.dismiss_scheduled.set(false);
    }
  })
  .forget();
}

/// The jump toast: shown while `server_jump` is set. "Jump" scrolls to the
/// server position (centered, like resume-on-open) and adopts it as the local
/// baseline; the ✕ dismisses without moving. Scrolling dismisses it too (see
/// [`note_scroll`]).
pub fn jump_toast(ctx: LiveCtx) -> impl IntoView {
  let jump = move |_| {
    if let Some(target) = ctx.server_jump.get_untracked()
      && let Some(el) = ctx.scroll_ref.get_untracked()
    {
      let top = (target as f64 * ctx.line_h()
        - ctx.viewport_h.get_untracked() / 2.0)
        .max(0.0);
      // Jumping adopts the peer's position; it is not the user moving. Suppress
      // the resulting scroll event's save so we don't echo the peer's own
      // position straight back to the server (the bug where a jump "updated the
      // progress on the server").
      ctx.suppress_next_scroll.set(true);
      el.set_scroll_top(top as i32);
      ctx.scroll_top.set(top);
    }
    // We now sit exactly where the peer's position pointed — adopting it, not
    // creating a new one — so clear `moved` and don't push it back.
    ctx.moved.set(false);
    // We are now at the server position, so nothing on the server is ahead of
    // us; adopt "now" as the baseline to stop this same push re-prompting.
    ctx.local_at.set(crate::clock::now_ms());
    ctx.dismiss_scheduled.set(false);
    ctx.server_jump.set(None);
  };
  let dismiss = move |_| {
    ctx.dismiss_scheduled.set(false);
    ctx.server_jump.set(None);
  };
  move || {
    ctx.server_jump.get().map(|_| {
      view! {
        <div class="toast" role="status">
          <span class="toast__text">
            "Reading position updated on another device."
          </span>
          <button class="toast__action" on:click=jump>"Jump"</button>
          <button class="toast__dismiss" aria-label="Dismiss"
            on:click=dismiss>"\u{2715}"</button>
        </div>
      }
    })
  }
}
