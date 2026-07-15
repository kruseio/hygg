//! The virtualized line/figure renderer for the reader's visible window.

use leptos::prelude::*;

use crate::ansi::ansi_to_html;
use crate::assets::ImageAsset;
use crate::model::{Book, LineKind};
use crate::settings::ImageMode;

/// Render the visible window as block-aware views: in "Images" mode a
/// figure/table asset draws as one centered `<img>` spanning its lines (its
/// fixed height keeps the scroll/anchor math exact); otherwise every line
/// renders on its own — blank (None), colored ASCII art (Ascii / the Images
/// fallback), or text. Returns the snapped first line (so the caller sets the
/// matching translateY offset) and the views.
#[allow(clippy::too_many_arguments)]
pub fn render_window(
  bk: &Book,
  assets: &[ImageAsset],
  mode: ImageMode,
  raw_first: usize,
  count: usize,
  speaking: Option<usize>,
  lh: f64,
  col_w: f64,
) -> (usize, Vec<AnyView>) {
  let total = bk.lines.len();
  let assets: &[ImageAsset] =
    if mode == ImageMode::Images { assets } else { &[] };
  let first = snap_first(assets, raw_first);
  let last = snap_last(assets, (raw_first + count).min(total), total);
  let mut views = Vec::new();
  let mut i = first;
  while i < last {
    if let Some(a) = assets.iter().find(|a| a.line_start == i) {
      let h = a.line_count as f64 * lh;
      views.push(
        view! {
          <img class="rline-img" src=a.data_url.clone()
            style=format!("height:{h}px;width:{col_w}px")/>
        }
        .into_any(),
      );
      i += a.line_count.max(1);
    } else {
      views.push(line_view(bk, i, speaking, mode));
      i += 1;
    }
  }
  (first, views)
}

/// One document line: blank (a hidden image row in None mode), colored ASCII
/// art, or text — with the narrated line marked.
fn line_view(
  bk: &Book,
  i: usize,
  speaking: Option<usize>,
  mode: ImageMode,
) -> AnyView {
  let spoken = if speaking == Some(i) { " rline--speaking" } else { "" };
  if matches!(bk.kinds.get(i), Some(LineKind::Ansi)) {
    if mode == ImageMode::None {
      return view! { <div class=format!("rline{spoken}")></div> }.into_any();
    }
    return view! {
      <div class=format!("rline rline--ansi{spoken}")
        inner_html=ansi_to_html(&bk.lines[i])/>
    }
    .into_any();
  }
  view! { <div class=format!("rline{spoken}")>{bk.lines[i].clone()}</div> }
    .into_any()
}

/// Pull `first` back to an image block's start when it lands inside one, so the
/// block renders from its top rather than being clipped by the window.
fn snap_first(assets: &[ImageAsset], first: usize) -> usize {
  assets
    .iter()
    .find(|a| a.line_start < first && first < a.line_start + a.line_count)
    .map_or(first, |a| a.line_start)
}

/// Push `last` out to an image block's end when it lands inside one.
fn snap_last(assets: &[ImageAsset], last: usize, total: usize) -> usize {
  let mut last = last.min(total);
  for a in assets {
    let end = a.line_start + a.line_count;
    if a.line_start < last && last < end {
      last = end.min(total);
    }
  }
  last
}
