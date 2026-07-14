//! The reader's scrollable body: the loading/error states and the virtualized
//! window of lines/figures. Split out of `reader.rs` to keep it within the LOC
//! budget; the reactive reads happen inside `reader_body`, called from the
//! reader's content closure, so tracking is identical to inlining it.

use leptos::prelude::*;

use super::reader_support::render_window;
use crate::app::SettingsCtx;
use crate::assets::ImageAsset;
use crate::model::Book;
use hygg_shared::sync::proto::DenialBody;

/// Extra lines rendered above and below the viewport, so a fast flick never
/// outruns the virtualized window and flashes blank rows.
pub const OVERSCAN: usize = 8;

/// Build the scroll body: while the book is loading show the spinner (or a load
/// error with an optional upgrade link); once loaded, render the virtualized
/// window around the current scroll position. `line_h` is the reader's fitted
/// line height, evaluated only past the loading guard so its reactive deps are
/// tracked exactly as when this lived inline in the reader.
#[allow(clippy::too_many_arguments)]
pub fn reader_body(
  book: RwSignal<Option<Book>>,
  load_error: RwSignal<Option<String>>,
  denial: RwSignal<Option<DenialBody>>,
  image_assets: RwSignal<Vec<ImageAsset>>,
  settings: SettingsCtx,
  scroll_top: RwSignal<f64>,
  viewport_h: RwSignal<f64>,
  viewport_w: RwSignal<f64>,
  speaking_line: RwSignal<Option<usize>>,
  line_h: &dyn Fn() -> f64,
) -> AnyView {
  if book.with(|b| b.is_none()) {
    return match load_error.get() {
      Some(msg) => {
        // The link and its wording both come from the server; the reader
        // states nothing about why it was refused.
        let action = denial
          .get()
          .and_then(|d| Some((d.action_url.clone()?, d.action_label.clone()?)));
        view! {
          <div class="pad reader__error">
            <p>{msg}</p>
            {action.map(|(url, label)| view! {
              <a class="reader__action" href=url target="_blank"
                rel="noopener">{label}</a>
            })}
          </div>
        }
        .into_any()
      }
      None => view! { <p class="pad">"Loading…"</p> }.into_any(),
    };
  }
  let lh = line_h();
  let total = book.with(|b| b.as_ref().map_or(0, |x| x.lines.len()));
  let pad = total as f64 * lh;
  let st = scroll_top.get();
  let vh = viewport_h.get().max(1.0);
  let raw_first = ((st / lh).floor() as usize).saturating_sub(OVERSCAN);
  let count = (vh / lh).ceil() as usize + OVERSCAN * 2;
  let speaking = speaking_line.get();
  let mode = settings.with(|s| s.image_mode);
  // Match the reader column width so figures align with the text.
  let col_w = (viewport_w.get() * 0.96).min(880.0);
  let (first, views) = book.with(|b| {
    let bk = b.as_ref().expect("book present past the None guard");
    image_assets.with(|a| {
      render_window(bk, a, mode, raw_first, count, speaking, lh, col_w)
    })
  });
  let offset = first as f64 * lh;
  view! {
    <div class="reader__pad" style=format!("height:{pad}px")>
      // Fixed `col_w`-wide and centered as one block (see `.reader__win`), so
      // every line's relative indentation survives — code blocks and ASCII art
      // keep their shape instead of each line centering on its own width.
      <div class="reader__win"
        style=format!("width:{col_w}px;transform:translateY({offset}px)")>
        {views}
      </div>
    </div>
  }
  .into_any()
}
