//! Free helpers for the reader: sync credentials, the virtualized line
//! renderer, and the throttled progress save/push. Split out to keep
//! `reader.rs` within the LOC budget.

mod image;
pub mod live;
pub mod mount;
mod progress;
mod render;

pub use image::ImageLoader;
pub use progress::{
  persist_on_exit, persist_progress, save_progress_throttled,
};
pub use render::render_window;

use crate::model::Book;
use crate::settings::Settings;
use crate::{storage, sync};

/// Sync credentials when the master switch is on and connected: the full
/// request credentials plus the server-assigned device id (used to tag pushed
/// ops). The auto-sync *scope* gates which documents push, per-document.
pub type Creds = (sync::Creds, String);

pub fn push_creds(settings: &Settings) -> Option<Creds> {
  let creds = settings.sync_creds()?;
  let device = settings.device_id.clone().unwrap_or_default();
  Some((creds, device))
}

/// Download a document's bytes on demand and turn them into a full, openable
/// book, caching it so later opens are instant. Used when the reader is opened
/// for a document whose content the background sync hasn't fetched yet (only
/// its metadata is local). Title/format come from the stored summary; falls
/// back to the id. Formats the browser can't extract fall back to the server's
/// conversion of the same stored document — which the server may decline
/// (surfaced as [`import_flow::DownloadError::Denied`]).
pub async fn fetch_book(
  creds: &sync::Creds,
  id: &str,
  col: usize,
) -> Result<Book, super::import_flow::DownloadError> {
  let bytes = sync::download_blob(creds, id)
    .await
    .map_err(super::import_flow::DownloadError::Unavailable)?;
  let filename = storage::get_summary(id)
    .await
    .map(|s| format!("{}.{}", s.title, s.format))
    .unwrap_or_else(|| id.to_string());
  let book =
    super::import_flow::book_from_download(creds, id, &filename, &bytes, col)
      .await?;
  let _ = storage::put_book(&book, &bytes).await;
  Ok(book)
}
