//! Reader: a virtualized, smooth-scrolling monospace column rendering the same
//! hygg justified text (plus inline colored ASCII-art rows). Only the lines
//! within the viewport (± overscan) are built each frame — spacers above and
//! below preserve the true scroll height — so even a thousand-page PDF stays
//! responsive. A corner pill shows the live reading percentage.

use iced::widget::text::{LineHeight, Wrapping};
use iced::widget::{
  Space, column, container, image, mouse_area, rich_text, row, scrollable,
  stack, text,
};
use iced::{Alignment, ContentFit, Length, Pixels};

use super::{Element, top_bar};
use crate::ansi::ansi_to_spans;
use crate::app::{
  HyggGui, MenuCtx, Message, OVERSCAN, Reader, TOPBAR_H, line_percent,
};
use crate::assets::ImageAsset;
use crate::layout;
use crate::model::LineKind;
use crate::select;
use crate::settings::ImageMode;
use crate::theme::{Palette, style};

/// The reader scrollable's stable id, so the app can restore the scroll offset
/// to the saved reading position after the document loads.
pub fn scroll_id() -> scrollable::Id {
  scrollable::Id::new("hygg-reader")
}

pub fn view<'a>(
  state: &'a HyggGui,
  r: &'a Reader,
  p: Palette,
  width: f32,
  vh: f32,
) -> Element<'a> {
  let title = r.book.as_ref().map(|b| b.title.clone()).unwrap_or_default();
  let bar =
    top_bar(p, title, Some(Message::GoHome), None, Some(Message::OpenSettings));

  let body: Element = match (&r.book, &r.error) {
    (_, Some(err)) => center_message(p, err, p.accent),
    (None, None) => center_message(p, "Loading…", p.muted),
    (Some(book), None) => {
      let zoom = state.settings().text_zoom as f64;
      let font = layout::fit_font_px(width as f64, book.col.max(1), zoom);
      let lh = layout::line_height(font) as f32;
      let total = book.lines.len();
      // The active text selection (normalized), highlighted line-by-line below.
      let sel = match (r.sel_anchor, r.sel_cursor) {
        (Some(a), Some(c)) => select::normalize(a, c),
        _ => None,
      };

      // Rasterized figures/tables only participate in "Images" mode; the other
      // modes keep the reader a pure per-line column (no multi-line blocks).
      let mode = state.settings().image_mode;
      let assets: &[ImageAsset] =
        if mode == ImageMode::Images { &r.assets } else { &[] };

      // The content is inset by the top bar's height (the bar overlays it), so
      // account for that inset when picking the virtualized window of lines.
      let raw_first = (((r.scroll_y - TOPBAR_H).max(0.0) / lh).floor()
        as usize)
        .saturating_sub(OVERSCAN);
      let count = ((vh + TOPBAR_H) / lh).ceil() as usize + OVERSCAN * 2;
      // Snap the window to whole image blocks so a partially-scrolled figure
      // still draws from its top; each block's fixed height (its line count ×
      // line height) keeps the scroll/anchor math exact.
      let first = snap_first(assets, raw_first);
      let last = snap_last(assets, (raw_first + count).min(total), total);
      let top_pad = TOPBAR_H + first as f32 * lh;
      let bottom_pad = total.saturating_sub(last) as f32 * lh;
      let col_w = layout::block_width(book.col, font, width);
      // One shared left margin centers the column as a block, so every line's
      // relative indentation survives — mirroring the TUI's single
      // `(width - col) / 2` offset (`select.rs` inverts the same margin).
      // Per-line centering instead mangles code blocks and ASCII art — the bug.
      let side_margin = ((width - col_w) / 2.0).max(0.0);

      // Fixed `col_w`-wide and left-aligned: lines sit at the left edge
      // (leading spaces intact) and the width stays stable as lines
      // scroll — no jitter.
      let mut win = column![].width(Length::Fixed(col_w));
      win = win.push(Space::with_height(Length::Fixed(top_pad)));
      let mut i = first;
      while i < last {
        if let Some(a) = asset_starting_at(assets, i) {
          win = win.push(image_block(&a.handle, a.line_count, lh, col_w));
          i += a.line_count.max(1);
        } else {
          win = win.push(line_widget(book, i, font, lh, p, sel, mode));
          i += 1;
        }
      }
      win = win.push(Space::with_height(Length::Fixed(bottom_pad)));

      // A left inset centers the block: the scrollable sizes content to its
      // natural width, so an inner `align_x(Center)` can't. The scrollbar still
      // sits at the viewport's right edge.
      let scroller = scrollable(
        container(win).padding(iced::Padding::ZERO.left(side_margin)),
      )
      .id(scroll_id())
      .on_scroll(Message::Scrolled)
      .width(Length::Fill)
      .height(Length::Fill);

      let pct =
        line_percent(center_line(r, book, lh, vh), total).round() as i64;
      let pill = container(
        container(text(format!("{pct}%")).size(13).color(p.fg))
          .style(style::pill(p))
          .padding([3, 9]),
      )
      .width(Length::Fill)
      .height(Length::Fill)
      .align_x(Alignment::End)
      .align_y(Alignment::End)
      .padding(16);

      stack![scroller, pill].into()
    }
  };

  // Browser-style selection over the body, plus a right-click menu. `on_move`
  // (position relative to the reader) tracks the pointer so a press / right-
  // click knows where it landed — single click / drag, double-click word,
  // triple-click line, Shift-click extend (see `HyggGui::reader_press`).
  let mut area = mouse_area(body)
    .on_press(Message::SelectStart)
    .on_release(Message::SelectEnd)
    .on_right_press(Message::OpenMenu(MenuCtx::Reader))
    .on_move(Message::SelectMove);
  // Show the I-beam (text) cursor over an open document so the prose reads as
  // selectable — the scrollbar keeps its own cursor. (Skipped for the
  // loading/error placeholders, which aren't selectable.)
  if r.book.is_some() {
    area = area.interaction(iced::mouse::Interaction::Text);
  }
  let area: Element = area.into();

  // The top bar overlays the reader and slides up on scroll-down (the content
  // inset above keeps text clear of it). `nav_offset` is the animated slide (0
  // = shown, TOPBAR_H = fully hidden); render it translated up via negative
  // top padding and clipped, so it slides rather than squishes. The context
  // menu is overlaid at the app level (see `HyggGui::view`).
  let offset = r.nav_offset.clamp(0.0, TOPBAR_H);
  let bar_layer = (offset < TOPBAR_H - 0.5).then(|| {
    container(bar)
      .width(Length::Fill)
      .padding(iced::Padding {
        top: -offset,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
      })
      .clip(true)
  });
  let stacked = stack![area].push_maybe(bar_layer);

  container(stacked)
    .style(style::app(p))
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// The document line at the vertical center of the viewport.
fn center_line(
  r: &Reader,
  book: &crate::model::Book,
  lh: f32,
  vh: f32,
) -> usize {
  let _ = book;
  (((r.scroll_y + vh / 2.0) / lh).floor()).max(0.0) as usize
}

/// A rasterized figure/table drawn over `count` document lines. Its fixed
/// height matches the vertical space those lines' text/ASCII rows would occupy,
/// so toggling image modes never shifts the document; `Contain` preserves the
/// image's aspect and centers it within the reading column.
fn image_block<'a>(
  handle: &iced::widget::image::Handle,
  count: usize,
  lh: f32,
  col_w: f32,
) -> Element<'a> {
  image(handle.clone())
    .width(Length::Fixed(col_w))
    .height(Length::Fixed(count as f32 * lh))
    .content_fit(ContentFit::Contain)
    .into()
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

/// The image block that begins exactly at `line`, if any.
fn asset_starting_at(
  assets: &[ImageAsset],
  line: usize,
) -> Option<&ImageAsset> {
  assets.iter().find(|a| a.line_start == line)
}

/// Render one document line: plain justified prose (with any selected columns
/// highlighted), or an image row per the current mode (blank in None; colored
/// ANSI-art spans in ASCII, and as the Images fallback until a raster loads).
/// `sel` is the normalized document selection, if any.
fn line_widget<'a>(
  book: &'a crate::model::Book,
  i: usize,
  font: f64,
  lh: f32,
  p: Palette,
  sel: Option<(select::Pos, select::Pos)>,
  mode: ImageMode,
) -> Element<'a> {
  let line = &book.lines[i];
  if matches!(book.kinds.get(i), Some(LineKind::Ansi)) {
    // "None" hides the art but keeps its height so no position moves.
    if mode == ImageMode::None {
      return Space::with_height(Length::Fixed(lh)).into();
    }
    // ANSI-art rows are images; they stay copyable (their raw text is included)
    // but aren't sub-range highlighted.
    let spans = ansi_to_spans(line, p.fg);
    let el: iced::Element<'a, ()> = rich_text(spans)
      .size(font as f32)
      .font(layout::MONO)
      .line_height(LineHeight::Absolute(Pixels(lh)))
      .into();
    return el.map(|_| Message::Noop);
  }
  let len = line.chars().count();
  let txt = text(line.as_str())
    .size(font as f32)
    .font(layout::MONO)
    .line_height(LineHeight::Absolute(Pixels(lh)))
    .wrapping(Wrapping::None)
    .color(p.fg);
  match sel.and_then(|s| select::cols_on_line(s, i, len)) {
    Some((s, e)) => {
      // Draw the selection as a background rectangle placed by column
      // arithmetic (monospace) behind the unbroken glyph run, so the text
      // never reflows. The three widths sum to the line width, so the
      // background row is exactly as wide as the text and left-aligns with it
      // inside the block-centered column.
      let adv = layout::char_advance(font) as f32;
      let hl = container(Space::new(
        Length::Fixed((e - s) as f32 * adv),
        Length::Fixed(lh),
      ))
      .style(style::selection(p));
      let bg = row![
        Space::with_width(Length::Fixed(s as f32 * adv)),
        hl,
        Space::with_width(Length::Fixed((len - e) as f32 * adv)),
      ];
      stack![bg, txt].into()
    }
    None => txt.into(),
  }
}

fn center_message<'a>(
  p: Palette,
  msg: &'a str,
  color: iced::Color,
) -> Element<'a> {
  let _ = p;
  container(text(msg).size(15).color(color))
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .into()
}
