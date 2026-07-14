//! Reader: a virtualized, smooth-scrolling monospace column rendering the same
//! hygg justified text (plus inline ASCII-art rows). Touch-first — native
//! momentum scroll, a top bar that hides on scroll-down, and automatic
//! progress save/restore. No keyboard/vim surface.

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_params_map;

use super::reader_support::{
  ImageLoader, live, mount, persist_on_exit, push_creds,
  save_progress_throttled,
};
use super::reader_view::reader_body;
use crate::app::SettingsCtx;
use crate::assets::ImageAsset;
use crate::components::TopBar;
use crate::model::Book;
use crate::{layout, tts};
use hygg_shared::sync::proto::DenialBody;

#[component]
pub fn Reader() -> impl IntoView {
  let params = use_params_map();
  let book_id = move || params.read().get("id").unwrap_or_default().to_string();

  let settings = expect_context::<SettingsCtx>();

  let book: RwSignal<Option<Book>> = RwSignal::new(None);
  // Rasterized figures/tables for "Images" mode, filled page by page as the
  // viewport reaches them (by `ImageLoader`, below). Kept sorted by line_start.
  let image_assets: RwSignal<Vec<ImageAsset>> = RwSignal::new(Vec::new());
  // Set when the document can't be loaded (not downloaded and the on-demand
  // server fetch failed), so the reader shows an error instead of a spinner.
  let load_error: RwSignal<Option<String>> = RwSignal::new(None);
  // Set alongside `load_error` when the server declined to convert the format:
  // its own wording and link, which the view shows as-is.
  let denial: RwSignal<Option<DenialBody>> = RwSignal::new(None);
  let scroll_top = RwSignal::new(0.0_f64);
  let viewport_h = RwSignal::new(0.0_f64);
  let viewport_w = RwSignal::new(0.0_f64);
  let nav_visible = RwSignal::new(true);
  let initial_line = RwSignal::new(0_usize);
  let restored = RwSignal::new(false);
  let last_save = RwSignal::new(0.0_f64);
  let speaking_line = RwSignal::new(None::<usize>);
  let tts_playing = RwSignal::new(false);
  let scroll_ref = NodeRef::<leptos::html::Div>::new();
  // Live cross-device position sync: a pending server jump (drives the toast),
  // the timestamp of the position we're showing (the last-write-wins baseline),
  // and whether a post-scroll dismiss is already queued.
  let server_jump = RwSignal::new(None::<usize>);
  let local_at = RwSignal::new(0.0_f64);
  let dismiss_scheduled = RwSignal::new(false);
  // Set true right before a *programmatic* scroll (open-restore, jump-to-peer),
  // so the native `scroll` event it triggers doesn't get mistaken for the user
  // moving and push the position back to the server (which would echo a peer's
  // own position at them, or overwrite newer progress). Consumed by
  // `on_scroll`.
  let suppress_next_scroll = RwSignal::new(false);
  // Whether the user has genuinely moved since the position was
  // restored/adopted. Gates the final save on exit: leaving without moving must
  // never push (a stale local position would clobber a newer peer's, since the
  // server does last-write-wins by timestamp).
  let moved = RwSignal::new(false);
  let live = live::LiveCtx {
    book,
    scroll_ref,
    scroll_top,
    viewport_h,
    viewport_w,
    settings,
    server_jump,
    local_at,
    dismiss_scheduled,
    suppress_next_scroll,
    moved,
  };

  // Auto-fit the column to the viewport width so it fills the screen and
  // centers (responsive). Line-height is 1.0 so ASCII-art half-blocks stack
  // seamlessly, matching the terminal.
  let font_px = move || {
    let col = book.with(|b| b.as_ref().map_or(64, |x| x.col.max(1)));
    layout::fit_font_px(
      viewport_w.get().max(1.0),
      col,
      settings.with(|s| s.text_zoom) as f64,
    )
  };
  let line_h = move || font_px().round().max(1.0);

  // The document line at the vertical center of the viewport. This is the
  // position we sync and restore, so the *same* content sits mid-screen on
  // every device — matching the CLI, which anchors on its centered cursor line
  // (syncing the top line instead lands the peer half a screen off).
  let center_line = move || {
    ((scroll_top.get() + viewport_h.get() / 2.0) / line_h()).floor().max(0.0)
      as usize
  };

  // Live reading percentage for the bottom-right corner indicator: the shared
  // width-independent character fraction of the center line, matching how the
  // CLI computes and shows it (and what syncs to the server). `None` until the
  // book is loaded, so nothing renders over the spinner.
  let percent = move || {
    book.with(|b| {
      b.as_ref()
        .filter(|x| !x.lines.is_empty())
        .map(|x| x.percent_of_line(center_line()).round() as i64)
    })
  };

  // Toggle narration: speak from the current top line, or stop.
  let toggle_tts = move |_| {
    if tts_playing.get_untracked() {
      tts::stop(tts_playing, speaking_line);
      return;
    }
    let lh = line_h();
    let from = (scroll_top.get_untracked() / lh).floor() as usize;
    tts::start(
      tts::SpeakCtx {
        book,
        speaking_line,
        playing: tts_playing,
        scroll_ref,
        rate: settings.with(|s| s.tts_rate),
        line_h: lh,
      },
      from,
    );
  };

  // Load the book + its saved position once on mount (logic in `mount`).
  mount::install_load_effect(
    book_id(),
    settings,
    initial_line,
    local_at,
    book,
    load_error,
    denial,
  );

  // Subscribe to live server pushes so a peer advancing this document raises
  // the jump toast instantly (the subscription is closed on unmount inside).
  live::install(live, book_id());

  // Once the container is mounted and the book is loaded, restore scroll —
  // instantly (no animation), jumping straight to the saved position.
  Effect::new(move |_| {
    if restored.get() {
      return;
    }
    if let (Some(el), Some(_)) = (scroll_ref.get(), book.get()) {
      viewport_w.set(el.client_width() as f64);
      let vh = el.client_height() as f64;
      viewport_h.set(vh);
      // Center the saved line in the viewport (it was measured at center too),
      // so the same content sits mid-screen as on the device it synced from.
      let target = (initial_line.get() as f64 * line_h() - vh / 2.0).max(0.0);
      // Restoring is not the user moving: suppress the resulting scroll event's
      // save so opening a document never pushes its position back.
      suppress_next_scroll.set(true);
      el.set_scroll_top(target as i32);
      scroll_top.set(target);
      restored.set(true);
    }
  });

  // Keep the fitted font in sync with viewport changes (rotation / resize).
  mount::install_resize_effect(scroll_ref, viewport_w, viewport_h);

  // Viewport-driven image loading in "Images" mode: parse the PDF once, then
  // decode only the pages that reach the screen.
  let loader = ImageLoader::new();
  loader.open_effect(book, settings, book_id());
  // Decode the visible pages whenever the source opens or the view scrolls.
  Effect::new(move |_| {
    if settings.with(|s| s.image_mode) != crate::settings::ImageMode::Images
      || !loader.ready()
    {
      return;
    }
    let lh = line_h();
    let st = scroll_top.get();
    let vh = viewport_h.get().max(1.0);
    let first = (st / lh).floor().max(0.0) as usize;
    let last = ((st + vh) / lh).ceil() as usize;
    loader.ensure(book, image_assets, first, last);
  });

  let on_scroll = move |_| {
    let Some(el) = scroll_ref.get() else { return };
    let st = el.scroll_top() as f64;
    let prev = scroll_top.get_untracked();
    let lh = line_h();
    if st > prev + 6.0 && st > lh * 1.5 {
      nav_visible.set(false);
    } else if st < prev - 6.0 {
      nav_visible.set(true);
    }
    scroll_top.set(st);
    let vh = el.client_height() as f64;
    viewport_h.set(vh);
    // A programmatic scroll (open-restore, jump-to-peer) is not the user
    // moving: keep the viewport/nav in sync but never save or push it —
    // that would echo a peer's own position back or overwrite newer
    // progress.
    if suppress_next_scroll.get_untracked() {
      suppress_next_scroll.set(false);
      return;
    }
    moved.set(true);
    let center = ((st + vh / 2.0) / lh).floor().max(0.0) as usize;
    save_progress_throttled(
      book_id(),
      book,
      center,
      last_save,
      settings.with(push_creds),
      settings.with(|s| s.auto_sync_scope),
    );
    // Scrolling = "keep my place": refresh the baseline and, if the jump toast
    // is up, start its dismiss grace (the save above already pushed our
    // position, so the server adopts it last-write-wins).
    live::note_scroll(live);
  };

  // Final save on leaving the reader (and stop any narration). Only flush the
  // position when the user actually moved: a clean exit from a restored/adopted
  // position must not push (last-write-wins would let a stale local position
  // clobber a peer's newer one — the back-button regression).
  let id_for_cleanup = book_id();
  on_cleanup(move || {
    tts::stop(tts_playing, speaking_line);
    if !moved.get_untracked() {
      return;
    }
    persist_on_exit(
      id_for_cleanup.clone(),
      book,
      scroll_top,
      viewport_h,
      viewport_w,
      settings,
    );
  });

  view! {
    <div class="reader">
      <TopBar
        title=Signal::derive(move || {
          book.with(|b| b.as_ref().map(|x| x.title.clone()).unwrap_or_default())
        })
        visible=nav_visible
        back_href="/".to_string()
      >
        <button class="iconbtn iconbtn--ring"
          class:iconbtn--on=move || tts_playing.get()
          on:click=toggle_tts aria-label="Read aloud">
          {move || if tts_playing.get() {
            tts::pause_icon().into_any()
          } else {
            tts::play_icon().into_any()
          }}
        </button>
      </TopBar>
      <div class="reader__scroll" node_ref=scroll_ref on:scroll=on_scroll
        style=move || format!("--fs:{}px;--lh:{}px", font_px(), line_h())>
        {move || {
          reader_body(
            book,
            load_error,
            denial,
            image_assets,
            settings,
            scroll_top,
            viewport_h,
            viewport_w,
            speaking_line,
            &line_h,
          )
        }}
      </div>
      {move || {
        percent().map(|p| view! {
          <div class="reader__progress">{format!("{p}%")}</div>
        })
      }}
      {live::jump_toast(live)}
    </div>
  }
}
