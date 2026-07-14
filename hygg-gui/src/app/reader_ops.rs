//! Reader-screen state plus the geometry + text-selection methods on
//! [`HyggGui`], split out of `app/mod.rs` to keep it within the source LOC
//! budget. The methods map the fitted monospace layout to pixels (for text
//! selection). The asset-loading and progress-save methods live in the sibling
//! [`super::reader_assets`] module. `pub(super)` so `app::update` can call
//! them.

use iced::Task;

use super::{HyggGui, Message, TOPBAR_H};
use crate::layout;
use crate::model::Book;
use crate::select;

/// Reader-screen state (the currently open document + its scroll position).
#[derive(Default)]
pub struct Reader {
  pub id: String,
  pub book: Option<Book>,
  pub error: Option<String>,
  /// Rasterized figures/tables for the "Images" render mode, mapped onto the
  /// document's lines. Filled lazily, page by page, as the viewport reaches
  /// them; empty until then (so image rows fall back to ASCII art) and for
  /// non-PDF or text-only documents. Kept sorted by `line_start`.
  pub assets: Vec<crate::assets::ImageAsset>,
  /// The open PDF's live visual source (parsed once), from which viewport
  /// pages are decoded on demand. `None` until opened (other modes,
  /// non-PDFs, or the open in flight).
  pub source: Option<crate::assets::AssetSource>,
  /// Whether an [`crate::assets::open`] is in flight, so we open the source at
  /// most once.
  pub source_pending: bool,
  /// Per 1-based page, whether its visuals have been requested (so a page is
  /// decoded at most once, even if scrolled past repeatedly). Sized when the
  /// source opens.
  pub pages_done: Vec<bool>,
  pub scroll_y: f32,
  pub restored: bool,
  pub last_save_ms: f64,
  /// Text-selection endpoints (`(line, column)` char positions). See
  /// [`crate::select`].
  pub sel_anchor: Option<select::Pos>,
  pub sel_cursor: Option<select::Pos>,
  /// A selection drag is in progress (left button held).
  pub selecting: bool,
  /// The `(line, column)` under the pointer, tracked on every move so a press
  /// knows where it landed (browser-style click/double/triple selection).
  pub hover: Option<select::Pos>,
  /// Click accounting for double/triple-click: time + place of the last press
  /// and the consecutive-click count (1 = single, 2 = word, 3 = line).
  pub last_click_ms: f64,
  pub last_click: Option<select::Pos>,
  pub click_count: u8,
  /// Whether Shift is held (extends the selection on click, browser-style).
  pub shift_held: bool,
  /// Top bar auto-hide target: set while scrolling down (past ~1.5 lines from
  /// the top), cleared on scroll-up — mirrors the PWA. Default `false` =
  /// shown.
  pub nav_hidden: bool,
  /// Animated top-bar slide offset in pixels (0 = shown, `TOPBAR_H` = fully
  /// hidden); eased toward `nav_hidden`'s target each frame for a slide.
  pub nav_offset: f32,
}

impl Reader {
  /// The top bar's resting slide offset for the current state (0 shown,
  /// `TOPBAR_H` hidden) — the value [`nav_offset`] animates toward.
  pub(super) fn nav_target(&self) -> f32 {
    if self.nav_hidden { TOPBAR_H } else { 0.0 }
  }
}

impl HyggGui {
  /// The reader viewport size: full width, height minus the top bar.
  pub(super) fn reader_viewport(&self) -> (f32, f32) {
    (self.viewport.width, (self.viewport.height - TOPBAR_H).max(1.0))
  }

  /// The fitted monospace font size (px) the reader currently renders at.
  fn reader_font(&self, book: &Book) -> f64 {
    let (w, _) = self.reader_viewport();
    layout::fit_font_px(
      w as f64,
      book.col.max(1),
      self.settings.text_zoom as f64,
    )
  }

  /// Line height the reader is currently rendering at, from the fitted font.
  pub(super) fn line_height(&self, book: &Book) -> f32 {
    layout::line_height(self.reader_font(book)) as f32
  }

  /// Map a pointer position (relative to the scroll viewport's top-left) to a
  /// `(line, column)` in the open document, for text selection.
  pub(super) fn locate(&self, p: iced::Point) -> Option<select::Pos> {
    let book = self.reader.book.as_ref()?;
    let font = self.reader_font(book);
    let lh = layout::line_height(font) as f32;
    Some(select::locate(
      book,
      p.x,
      // The reader content is inset by the top bar's height (the bar overlays
      // it), so shift the pointer up by that inset before mapping to a line.
      p.y - TOPBAR_H,
      self.reader.scroll_y,
      self.viewport.width,
      font,
      lh,
    ))
  }

  /// Track the pointer as it moves over the reader: remember its pixel position
  /// (for a right-click menu anchor) and the `(line, col)` under it, and —
  /// while dragging — extend the selection to it.
  pub(super) fn reader_move(&mut self, p: iced::Point) {
    self.reader.hover = self.locate(p);
    if self.reader.selecting {
      self.reader.sel_cursor = self.reader.hover;
    }
  }

  /// React to a reader scroll: hide the top bar while scrolling down (once past
  /// ~1.5 lines from the top) and reveal it on scroll-up — mirrors the PWA's
  /// auto-hiding nav. A small threshold ignores scroll jitter.
  pub(super) fn on_reader_scroll(&mut self, y: f32) {
    let prev = self.reader.scroll_y;
    let lh =
      self.reader.book.as_ref().map(|b| self.line_height(b)).unwrap_or(1.0);
    if y > prev + 6.0 && y > lh * 1.5 {
      self.reader.nav_hidden = true;
    } else if y < prev - 6.0 {
      self.reader.nav_hidden = false;
    }
    self.reader.scroll_y = y;
  }

  /// Ease the top bar's slide one animation frame toward its target, snapping
  /// when close so it settles. Driven by the `window::frames` subscription,
  /// which the app includes only while the bar is in motion.
  pub(super) fn anim_step(&mut self) -> Task<Message> {
    let target = self.reader.nav_target();
    let d = target - self.reader.nav_offset;
    self.reader.nav_offset =
      if d.abs() <= 0.75 { target } else { self.reader.nav_offset + d * 0.22 };
    Task::none()
  }

  /// Select the whole open document (the reader's Select-all menu action).
  pub(super) fn reader_select_all(&mut self) {
    if let Some(book) = self.reader.book.as_ref() {
      let last = book.lines.len().saturating_sub(1);
      let end = book.lines.get(last).map_or(0, |l| l.chars().count());
      self.reader.sel_anchor = Some((0, 0));
      self.reader.sel_cursor = Some((last, end));
    }
  }

  /// Whether the reader has a non-empty selection (enables the menu's Copy).
  pub fn reader_has_selection(&self) -> bool {
    matches!(
      (self.reader.sel_anchor, self.reader.sel_cursor),
      (Some(a), Some(c)) if select::normalize(a, c).is_some()
    )
  }

  /// A press over the reader, browser-style: a single click starts a drag (or,
  /// with Shift, extends the current selection); a double click selects the
  /// word under the pointer; a triple click selects the whole line. The
  /// landing spot comes from the tracked hover.
  pub(super) fn reader_press(&mut self) {
    let Some(pos) = self.reader.hover else {
      return;
    };
    // Count consecutive clicks at (roughly) the same spot within the window.
    let now = crate::util::now_ms();
    let repeat = now - self.reader.last_click_ms < 450.0
      && self.reader.last_click.is_some_and(|(l, c)| {
        l == pos.0 && (c as i64 - pos.1 as i64).abs() <= 1
      });
    self.reader.click_count =
      if repeat { (self.reader.click_count + 1).min(3) } else { 1 };
    self.reader.last_click_ms = now;
    self.reader.last_click = Some(pos);

    // The clicked line's text (cloned so the mutable borrows below are free).
    let line = self
      .reader
      .book
      .as_ref()
      .and_then(|b| b.lines.get(pos.0))
      .cloned()
      .unwrap_or_default();

    match self.reader.click_count {
      2 => {
        let (s, e) = crate::select::word_bounds(&line, pos.1);
        self.reader.sel_anchor = Some((pos.0, s));
        self.reader.sel_cursor = Some((pos.0, e));
        self.reader.selecting = false;
      }
      3 => {
        self.reader.sel_anchor = Some((pos.0, 0));
        self.reader.sel_cursor = Some((pos.0, line.chars().count()));
        self.reader.selecting = false;
      }
      _ => {
        // Single click: Shift extends from the existing anchor; otherwise this
        // becomes the new anchor. A release with no drag leaves anchor ==
        // cursor (an empty selection), so a plain click clears any
        // prior selection.
        if !(self.reader.shift_held && self.reader.sel_anchor.is_some()) {
          self.reader.sel_anchor = Some(pos);
        }
        self.reader.sel_cursor = Some(pos);
        self.reader.selecting = true;
      }
    }
  }

  /// The currently selected reader text, or `None` when nothing is selected.
  pub(super) fn selection_text(&self) -> Option<String> {
    let book = self.reader.book.as_ref()?;
    let sel =
      select::normalize(self.reader.sel_anchor?, self.reader.sel_cursor?)?;
    Some(select::extract(book, sel))
  }
}
