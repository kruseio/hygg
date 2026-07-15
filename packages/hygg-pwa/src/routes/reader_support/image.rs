//! Viewport-driven image loading for the reader's "Images" mode.

use cli_pdf_to_text::PdfVisualExtractor;
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::app::SettingsCtx;
use crate::assets::ImageAsset;
use crate::model::Book;
use crate::settings::ImageMode;

/// Pages decoded ahead of the viewport's last visible page, so the next figure
/// is ready before it scrolls into view.
const LOOKAHEAD: usize = 2;

/// Viewport-driven image loader: parses the open PDF once into a live extractor
/// and decodes only the pages that reach the viewport (plus a small
/// look-ahead), caching which pages are done so scrolling back is instant.
/// Replaces decoding the whole document up front, which cost seconds before the
/// first figure appeared on a large book. `Copy` (all reactive handles), so the
/// reader threads it through effects and the scroll handler freely.
#[derive(Clone, Copy)]
pub struct ImageLoader {
  /// The parsed PDF; `None` until opened (or on a non-PDF / parse failure).
  extractor: StoredValue<Option<PdfVisualExtractor>>,
  /// Per 1-based page, whether its visuals have been requested. Sized (to
  /// `total_pages + 1`) when the extractor opens; its length also carries the
  /// page count.
  pages_done: StoredValue<Vec<bool>>,
  /// Flips true once the extractor is parsed, triggering the first decode.
  ready: RwSignal<bool>,
  /// An open is in flight, so we parse the document at most once.
  opening: RwSignal<bool>,
}

impl ImageLoader {
  pub fn new() -> Self {
    Self {
      extractor: StoredValue::new(None),
      pages_done: StoredValue::new(Vec::new()),
      ready: RwSignal::new(false),
      opening: RwSignal::new(false),
    }
  }

  pub fn ready(&self) -> bool {
    self.ready.get()
  }

  /// Parse the visual source once, when the reader is in "Images" mode and the
  /// book is a PDF. Re-checks reactively, so switching into Images mode later
  /// opens it then.
  pub fn open_effect(
    self,
    book: RwSignal<Option<Book>>,
    settings: SettingsCtx,
    id: String,
  ) {
    Effect::new(move |_| {
      let want = settings.with(|s| s.image_mode == ImageMode::Images);
      let ready = book.with(|b| b.as_ref().is_some_and(|x| x.format == "pdf"));
      if !want || !ready || self.opening.get_untracked() || self.ready() {
        return;
      }
      self.opening.set(true);
      let id = id.clone();
      spawn_local(async move {
        let ex = crate::assets::open(id).await;
        let total = ex.as_ref().map_or(0, |e| e.total_pages());
        self.extractor.set_value(ex);
        self.pages_done.set_value(vec![false; total + 1]);
        self.ready.set(true);
      });
    });
  }

  /// Decode the not-yet-requested pages overlapping the line range
  /// `[first_line, last_line]` (plus the look-ahead), cooperatively so a heavy
  /// page can't freeze the reader. Pages are marked requested up front, so
  /// overlapping scroll events never double-decode. No-op until the source is
  /// open.
  pub fn ensure(
    self,
    book: RwSignal<Option<Book>>,
    image_assets: RwSignal<Vec<ImageAsset>>,
    first_line: usize,
    last_line: usize,
  ) {
    if !self.ready.get_untracked() {
      return;
    }
    let total_pages = self.pages_done.with_value(|d| d.len().saturating_sub(1));
    if total_pages == 0 {
      return;
    }
    let (start, end) = book.with_untracked(|b| match b.as_ref() {
      Some(bk) => {
        let last = bk.lines.len().saturating_sub(1);
        let page = |line: usize| {
          bk.page_of_line(line.min(last)).map_or(1, |(p, _)| p as usize)
        };
        (page(first_line), (page(last_line) + LOOKAHEAD).min(total_pages))
      }
      None => (1, 0),
    });
    if start > end {
      return;
    }
    // Claim the pages to decode (marking them done) in one atomic update.
    let todo: Vec<usize> = self
      .pages_done
      .try_update_value(|d| {
        let todo: Vec<usize> = (start..=end)
          .filter(|&p| !d.get(p).copied().unwrap_or(true))
          .collect();
        for &p in &todo {
          d[p] = true;
        }
        todo
      })
      .unwrap_or_default();
    if todo.is_empty() {
      return;
    }
    spawn_local(async move {
      for page in todo {
        // Yield *before* decoding, not after: `spawn_local` runs as a
        // microtask, and microtasks run before the browser paints. A page
        // decode is synchronous wasm work (rasterize + PNG + base64), so
        // without this yield a jump into an undecoded page froze the old
        // frame for the whole first decode — the "lag" on jump-to-server-
        // position. The timeout is a macrotask, so the freshly scrolled
        // window paints first (ASCII rows as the placeholder), then each
        // page decodes between frames.
        TimeoutFuture::new(0).await;
        let assets = book.with_untracked(|b| {
          let Some(bk) = b.as_ref() else {
            return Vec::new();
          };
          self.extractor.with_value(|e| {
            e.as_ref()
              .map_or(Vec::new(), |ex| crate::assets::page_assets(ex, bk, page))
          })
        });
        if !assets.is_empty() {
          image_assets.update(|v| {
            v.extend(assets);
            v.sort_by_key(|a| a.line_start);
          });
        }
      }
    });
  }
}
