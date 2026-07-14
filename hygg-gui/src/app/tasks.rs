//! Async side-effects the `update` handler dispatches via `Task::perform`:
//! library/document loading, import, delete, progress persistence, and the file
//! picker / OS document handling. Split out of `app/mod.rs` for the LOC budget.

use std::collections::HashMap;
use std::path::PathBuf;

use hygg_shared::sync::proto::ProgressDto;

use super::{Message, line_percent};
use crate::model::{Book, BookSummary, Progress};
use crate::sync::{self, Creds};

pub(super) async fn load_library()
-> (Vec<BookSummary>, HashMap<String, Progress>) {
  let lib = crate::storage::list_library().await;
  let mut prog = HashMap::new();
  for b in &lib {
    prog.insert(b.id.clone(), crate::storage::get_progress(b.id.clone()).await);
  }
  (lib, prog)
}

pub(super) async fn load_reader(
  id: String,
  creds: Option<Creds>,
  col: usize,
) -> Result<(Book, Progress), String> {
  // Load the full book locally; if only its metadata is here (or nothing),
  // download the bytes from the server on demand so the reader still opens.
  let mut book = match crate::storage::get_book(id.clone()).await {
    Some(b) => b,
    None => match &creds {
      Some(creds) => fetch_book(creds, &id, col).await?,
      None => {
        return Err("This document isn't available offline yet.".to_string());
      }
    },
  };
  // Upgrade a PDF imported before page tracking existed: re-extract from the
  // cached source bytes so page-anchored resume/sync works.
  if book.format == "pdf"
    && !book.has_pages()
    && let Some(bytes) = crate::storage::get_blob(id.clone()).await
    && let Ok(upgraded) = crate::format::import(
      &format!("{}.pdf", book.title),
      &bytes,
      book.col.max(1),
    )
  {
    let _ = crate::storage::put_book(upgraded.clone(), bytes).await;
    book = upgraded;
  }
  let mut progress = crate::storage::get_progress(id.clone()).await;
  // When connected, adopt the server's position if it is newer than the local
  // one (last-write-wins) — resolving the pagination-independent word anchor to
  // this reader's own line, so you resume on the exact same content the peer
  // left off at. The adopted position is persisted so the reader restores it.
  if let Some(creds) = &creds
    && let Ok(rows) = sync::pull_progress(creds, None).await
    && let Some(p) = rows.iter().find(|p| p.book_id == id)
    && (p.updated_at as f64) > progress.updated_at
  {
    progress.line = adopt_line(&book, p);
    progress.percent = p.percentage;
    progress.updated_at = p.updated_at as f64;
    let _ = crate::storage::put_progress(id, progress).await;
  }
  Ok((book, progress))
}

/// Download a document's bytes and turn them into a full, openable book,
/// caching it. Title/format come from the stored metadata summary; falls back
/// to the id. A format the GUI can't extract falls back to server conversion.
async fn fetch_book(
  creds: &Creds,
  id: &str,
  col: usize,
) -> Result<Book, String> {
  let bytes = sync::download_blob(creds, id).await?;
  let filename = crate::storage::get_summary(id.to_string())
    .await
    .map(|s| format!("{}.{}", s.title, s.format))
    .unwrap_or_else(|| format!("{id}.txt"));
  let book = book_from_download(creds, id, &filename, &bytes, col).await?;
  let _ = crate::storage::put_book(book.clone(), bytes).await;
  Ok(book)
}

/// Turn a downloaded document's bytes into an openable book: local extraction
/// when the GUI can render the format, else the server's conversion of the same
/// stored document (DOCX, scanned PDFs). A server that declines to convert
/// explains itself, and that explanation becomes the error shown here.
async fn book_from_download(
  creds: &Creds,
  id: &str,
  filename: &str,
  bytes: &[u8],
  col: usize,
) -> Result<Book, String> {
  if let Ok(book) = crate::format::import(filename, bytes, col)
    && crate::format::has_text(&book)
  {
    return Ok(book);
  }
  match sync::fetch_extraction(creds, id, col).await {
    Ok(resp) => Ok(crate::format::book_from_server_text(
      &resp.title,
      &resp.format,
      &resp.text,
      bytes,
      col,
    )),
    // The server explained itself; append its link when it offered one.
    Err(sync::ExtractErr::Denied(body)) => Err(match body.action() {
      Some((url, label)) => format!("{} — {label}: {url}", body.error),
      None => body.error,
    }),
    Err(sync::ExtractErr::Failed(e)) => Err(e),
  }
}

/// Resolve a pulled server position to a line index in *this* reader's line
/// space: the width-independent character anchor first (same content at any
/// wrap width), then an exact line match, then a percentage remap.
fn adopt_line(book: &Book, p: &ProgressDto) -> usize {
  let total = book.lines.len();
  if let Some(w) = p.word_offset
    && w >= 0
  {
    return book.line_for_word(p.page.map(|pg| pg as u32), w as usize);
  }
  if p.total_lines as usize == total && p.offset_line >= 0 {
    return p.offset_line as usize;
  }
  if p.percentage > 0.0 && total > 0 {
    return ((p.percentage / 100.0) * total as f64).round() as usize;
  }
  p.offset_line.max(0) as usize
}

/// Background server sync for the Home screen: pull the account's library
/// (downloading any documents not held locally so they become openable) and its
/// reading positions (merging any that are newer than the local record). Runs
/// on open/refresh when connected; no-op offline.
pub(super) async fn sync_from_server(creds: Creds, col: usize) {
  if let Ok(books) = sync::list_books(&creds).await {
    for b in books {
      if !crate::storage::has_book(b.content_hash.clone()).await
        && let Ok(bytes) = sync::download_blob(&creds, &b.content_hash).await
      {
        let filename = format!("{}.{}", b.title, b.format);
        // Local extraction, else server conversion; a conversion refused for
        // a refusal just leaves it metadata-only (explained on open).
        if let Ok(book) =
          book_from_download(&creds, &b.content_hash, &filename, &bytes, col)
            .await
        {
          let _ = crate::storage::put_book(book, bytes).await;
        }
      }
    }
  }
  if let Ok(rows) = sync::pull_progress(&creds, None).await {
    for p in rows {
      let mut local = crate::storage::get_progress(p.book_id.clone()).await;
      if (p.updated_at as f64) > local.updated_at {
        local.line = p.offset_line.max(0) as usize;
        local.percent = p.percentage;
        local.updated_at = p.updated_at as f64;
        let _ = crate::storage::put_progress(p.book_id.clone(), local).await;
      }
    }
  }
}

pub(super) async fn import_bytes(
  name: String,
  bytes: Vec<u8>,
  col: usize,
) -> Result<String, String> {
  let book = crate::format::import(&name, &bytes, col)?;
  let id = book.id.clone();
  crate::storage::put_book(book, bytes).await?;
  Ok(id)
}

pub(super) async fn delete_book(id: String) {
  let _ = crate::storage::delete_book(id).await;
}

/// Merge a new position into stored progress, preserving accumulated seconds by
/// reading the latest record first (so concurrent throttled saves don't clobber
/// the counter), then adding `add`.
#[allow(clippy::too_many_arguments)]
pub(super) async fn persist_progress(
  id: String,
  total: usize,
  line: usize,
  add: f64,
  word_offset: Option<u64>,
  // 1-based PDF page + line-within-page for the anchored line; `None` for
  // reflowable formats. Keeps the invariant "anchor is page-local iff a page
  // is present" so a peer interprets `word_offset` correctly.
  page_anchor: Option<(u32, usize)>,
  creds: Option<Creds>,
  // Automatic-sync scope: gates whether this document pushes at all (combined
  // with its opt-in and the book heuristic).
  scope: hygg_shared::sync::AutoSyncPolicy,
) {
  let mut p = crate::storage::get_progress(id.clone()).await;
  p.line = line;
  p.percent = line_percent(line, total);
  p.seconds += add;
  p.updated_at = crate::util::now_ms();
  let _ = crate::storage::put_progress(id.clone(), p).await;
  // Best-effort push so a peer resumes here. A document pushes only when its
  // effective `SyncMode` permits state *and* the scope covers it (`off` keeps a
  // document local; a report the scope doesn't cover stays on this device).
  if let Some(creds) = creds {
    let syncs = crate::storage::get_summary(id.clone())
      .await
      .map(|s| s.effective_sync_mode().syncs_state() && s.auto_syncs(scope))
      .unwrap_or(false);
    if syncs {
      let (page, line_in_page) = match page_anchor {
        Some((pg, lip)) => (Some(pg), Some(lip as u64)),
        None => (None, None),
      };
      let _ = crate::sync::push_progress(
        &creds,
        &id,
        line as u64,
        total as u64,
        p.percent,
        page,
        line_in_page,
        word_offset,
      )
      .await;
    }
  }
}

/// Open the file picker (AppKit/GTK/Win32) and read the chosen document's
/// bytes.
pub(super) async fn pick_file() -> Option<(String, Vec<u8>)> {
  let handle = rfd::AsyncFileDialog::new()
    .add_filter("Documents", crate::format::SUPPORTED_EXTENSIONS)
    .pick_file()
    .await?;
  let name = handle.file_name();
  let bytes = handle.read().await;
  Some((name, bytes))
}

/// Read a document from a filesystem path and import it (OS file handler).
pub(super) async fn open_path(
  path: PathBuf,
  col: usize,
) -> Result<String, String> {
  let name =
    path.file_name().and_then(|s| s.to_str()).unwrap_or("document").to_string();
  let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
  import_bytes(name, bytes, col).await
}

/// Map raw window events to messages: OS document drops, Shift's held state
/// (extends the reader selection on click), and key presses (routed to the
/// focused text field or the reader in `update`).
pub(super) fn on_platform_event(
  event: iced::Event,
  _status: iced::event::Status,
  _id: iced::window::Id,
) -> Option<Message> {
  use iced::keyboard::Event::{KeyPressed, ModifiersChanged};
  match event {
    iced::Event::Window(iced::window::Event::FileDropped(path)) => {
      Some(Message::FileOpened(path))
    }
    iced::Event::Keyboard(ModifiersChanged(m)) => {
      Some(Message::SetShift(m.shift()))
    }
    iced::Event::Keyboard(KeyPressed { key, text, modifiers, .. }) => {
      Some(Message::KeyPressed(key, text.map(|s| s.to_string()), modifiers))
    }
    // Track the window cursor so a right-click can anchor its context menu.
    iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
      Some(Message::CursorMoved(position))
    }
    _ => None,
  }
}
