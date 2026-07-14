//! Reader asset loading (lazy PDF figure/table extraction) plus the
//! progress-save methods on [`HyggGui`], split out of [`super::reader_ops`] to
//! keep each file within the source LOC budget. `pub(super)` so `app::update`
//! can call them.

use iced::Task;

use super::tasks::{persist_progress, sync_from_server};
use super::{HyggGui, Message};
use crate::model::Book;

impl HyggGui {
  /// Prepare the open PDF's "Images" mode assets when wanted: open the live
  /// visual source once, then decode the pages the viewport currently shows. A
  /// no-op otherwise (other modes, non-PDFs), so it's cheap to call
  /// opportunistically on open and on switching into Images mode.
  pub(super) fn maybe_load_assets(&mut self) -> Task<Message> {
    if self.settings.image_mode != crate::settings::ImageMode::Images {
      return Task::none();
    }
    let Some(book) = self.reader.book.as_ref() else {
      return Task::none();
    };
    if book.format != "pdf" {
      return Task::none();
    }
    // Open the source once; page extraction follows on ready and on scroll.
    if self.reader.source.is_none() {
      if self.reader.source_pending {
        return Task::none();
      }
      self.reader.source_pending = true;
      let id = self.reader.id.clone();
      let tag = id.clone();
      return Task::perform(crate::assets::open(id), move |s| {
        Message::AssetSourceReady(tag.clone(), s)
      });
    }
    self.request_visible_pages()
  }

  /// Decode the not-yet-requested pages that overlap the current viewport (plus
  /// a small reading-direction look-ahead). No-op unless the source is open and
  /// Images mode is on, so it's safe to call on every scroll — nothing new to
  /// decode returns an empty task.
  pub(super) fn request_visible_pages(&mut self) -> Task<Message> {
    if self.settings.image_mode != crate::settings::ImageMode::Images {
      return Task::none();
    }
    let Some(src) = self.reader.source.clone() else {
      return Task::none();
    };
    let todo: Vec<usize> = {
      let Some(book) = self.reader.book.as_ref() else {
        return Task::none();
      };
      self
        .visible_pages(book, src.total_pages)
        .into_iter()
        .filter(|&p| !self.reader.pages_done.get(p).copied().unwrap_or(true))
        .collect()
    };
    if todo.is_empty() {
      return Task::none();
    }
    for &p in &todo {
      self.reader.pages_done[p] = true;
    }
    let tag = self.reader.id.clone();
    Task::perform(crate::assets::extract_pages(src, todo), move |a| {
      Message::AssetsLoaded(tag.clone(), a)
    })
  }

  /// The 1-based pages whose flattened lines overlap the viewport, extended by
  /// a couple of pages ahead so the next figure is ready before it scrolls
  /// in.
  fn visible_pages(&self, book: &Book, total_pages: usize) -> Vec<usize> {
    const LOOKAHEAD: usize = 2;
    let lh = self.line_height(book).max(1.0);
    let (_, vh) = self.reader_viewport();
    let last_line = book.lines.len().saturating_sub(1);
    let first = (self.reader.scroll_y / lh).floor().max(0.0) as usize;
    let last =
      (((self.reader.scroll_y + vh) / lh).ceil() as usize).min(last_line);
    let page =
      |line: usize| book.page_of_line(line).map_or(1, |(p, _)| p as usize);
    let start = page(first.min(last_line));
    let end = (page(last) + LOOKAHEAD).min(total_pages.max(1));
    (start..=end).collect()
  }

  /// Pull the account's library + positions from the server, then refresh home.
  pub(super) fn resync(&self, creds: crate::sync::Creds) -> Task<Message> {
    let col = self.settings.import_col;
    Task::perform(sync_from_server(creds, col), |_| Message::ServerSynced)
  }

  /// The document line at the vertical center of the viewport — the anchor we
  /// persist and restore, matching the PWA and CLI.
  fn center_line(&self, book: &Book) -> usize {
    let lh = self.line_height(book);
    let (_, vh) = self.reader_viewport();
    (((self.reader.scroll_y + vh / 2.0) / lh).floor()).max(0.0) as usize
  }

  /// Persist the current reader position immediately (used when leaving).
  pub(super) fn save_reader_progress(&mut self) -> Task<Message> {
    let id = self.reader.id.clone();
    let creds = self.settings.creds();
    let Some(book) = self.reader.book.as_ref() else {
      return Task::none();
    };
    let total = book.lines.len();
    let line = self.center_line(book);
    let word = Some(book.word_offset_of_line(line) as u64);
    // For PDFs the anchor is page-local, so the page it belongs to must ride
    // along — otherwise a peer resolves the page-local offset globally and
    // lands on the wrong page.
    let page = book.page_of_line(line);
    let scope = self.settings.auto_sync_scope;
    Task::perform(
      persist_progress(id, total, line, 0.0, word, page, creds, scope),
      |_| Message::Noop,
    )
  }

  /// Throttled progress save while scrolling (≤ ~1.4/s), also accumulating
  /// active reading seconds since the previous save (idle-capped).
  pub(super) fn save_reader_progress_throttled(&mut self) -> Task<Message> {
    let now = crate::util::now_ms();
    let prev = self.reader.last_save_ms;
    if now - prev < 700.0 {
      return Task::none();
    }
    self.reader.last_save_ms = now;
    let id = self.reader.id.clone();
    let creds = self.settings.creds();
    let Some(book) = self.reader.book.as_ref() else {
      return Task::none();
    };
    let total = book.lines.len();
    let line = self.center_line(book);
    let word = Some(book.word_offset_of_line(line) as u64);
    let page = book.page_of_line(line);
    let add = if prev > 0.0 { ((now - prev) / 1000.0).min(60.0) } else { 0.0 };
    let scope = self.settings.auto_sync_scope;
    Task::perform(
      persist_progress(id, total, line, add, word, page, creds, scope),
      |_| Message::Noop,
    )
  }
}
