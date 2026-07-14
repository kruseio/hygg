//! Mount-time effects for the reader: the one-shot book+position load and the
//! window resize listener. Split out of `reader.rs` to keep it within the LOC
//! budget; both just wire signals to effects, holding no state of their own.

use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

use super::push_creds;
use crate::app::SettingsCtx;
use crate::model::Book;
use hygg_shared::sync::proto::DenialBody;

/// Load the book + its saved position once on mount. Resolves the document
/// (local cache or on-demand server fetch), seeds the resume line and the live
/// last-write-wins baseline timestamp, and — when the document can't be opened
/// — surfaces the load error and optional upgrade nudge instead of a spinner.
#[allow(clippy::too_many_arguments)]
pub fn install_load_effect(
  book_id: String,
  settings: SettingsCtx,
  initial_line: RwSignal<usize>,
  local_at: RwSignal<f64>,
  book: RwSignal<Option<Book>>,
  load_error: RwSignal<Option<String>>,
  denial: RwSignal<Option<DenialBody>>,
) {
  Effect::new(move |prev: Option<()>| {
    if prev.is_some() {
      return;
    }
    let id = book_id.clone();
    let creds = settings.with(|s| push_creds(s).map(|(creds, _)| creds));
    let col = settings.with(|s| s.import_col);
    spawn_local(async move {
      let r = super::super::reader_load::resolve(id, creds, col).await;
      initial_line.set(r.initial_line);
      // Seed the live last-write-wins baseline with the restored position's
      // timestamp so a peer that later advances the document reads as newer.
      local_at.set(r.position_updated_at);
      match r.book {
        Some(b) => book.set(Some(b)),
        None => {
          load_error.set(r.error);
          denial.set(r.denial);
        }
      }
    });
  });
}

/// Keep the fitted font in sync with viewport changes (rotation / resize) by
/// re-measuring the scroll container on the window `resize` event.
pub fn install_resize_effect(
  scroll_ref: NodeRef<leptos::html::Div>,
  viewport_w: RwSignal<f64>,
  viewport_h: RwSignal<f64>,
) {
  Effect::new(move |prev: Option<()>| {
    if prev.is_some() {
      return;
    }
    if let Some(win) = web_sys::window() {
      let cb = Closure::<dyn FnMut()>::new(move || {
        if let Some(el) = scroll_ref.get_untracked() {
          viewport_w.set(el.client_width() as f64);
          viewport_h.set(el.client_height() as f64);
        }
      });
      let _ = win.add_event_listener_with_callback(
        "resize",
        cb.as_ref().unchecked_ref(),
      );
      cb.forget();
    }
  });
}
