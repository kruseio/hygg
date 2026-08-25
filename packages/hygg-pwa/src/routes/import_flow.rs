//! Import pipeline shared by Home: try client-side extraction first, then fall
//! back to the server for formats the browser can't handle (scanned-PDF OCR,
//! pandoc/DOCX). Pure async logic returning an [`ImportResult`] the view turns
//! into status/banners — keeps the component body small.
//!
//! A server that declines to convert explains itself; nothing here writes that
//! wording, it only carries it to the view.

use hygg_shared::sync::proto::DenialBody;

use crate::format::{book_from_server_text, import};
use crate::model::Book;
use crate::storage;
use crate::sync::{self, ConvertErr};

/// Outcome of importing one file.
pub enum ImportResult {
  /// Saved locally (and uploaded if connected).
  Saved,
  /// A user-facing status/error message.
  Message(String),
  /// The browser can't render this format and the server declined to convert
  /// it. Carries the server's own explanation and optional link, shown as-is —
  /// a server that converts for everyone never produces this.
  Denied(DenialBody),
}

/// Why turning a *downloaded* document into an openable book failed.
pub enum DownloadError {
  /// The browser can't render this format and the server declined to convert
  /// it, in its own words.
  Denied(DenialBody),
  /// The document couldn't be loaded/rendered for another reason (network,
  /// unconvertible format, …).
  Unavailable(String),
}

/// Full sync credentials when connected + auto-sync is on.
pub type Creds = Option<sync::Creds>;

/// Import a file: client-side when possible, else server `/convert`.
pub async fn do_import(
  name: String,
  bytes: Vec<u8>,
  col: usize,
  creds: Creds,
  scope: hygg_shared::sync::AutoSyncPolicy,
) -> ImportResult {
  let client = import(&name, &bytes, col).await;

  // Use the client result only if it actually produced readable text — a
  // scanned PDF extracts to (near) nothing and should fall through to OCR.
  if let Ok(book) = &client
    && has_text(book)
  {
    return finish(book, &bytes, &creds, scope).await;
  }

  let Some(creds) = creds else {
    return ImportResult::Message(match client {
      Err(msg) => msg,
      Ok(_) => "No text found. Connect a server to OCR this document.".into(),
    });
  };

  // Server-side extraction would have to receive the plaintext bytes, which
  // would defeat end-to-end encryption. When a key is set up, refuse rather
  // than leak: this format needs a client that can extract it locally.
  if creds.key.is_some() {
    return ImportResult::Message(
      "This document needs local extraction (e.g. scanned-PDF OCR), which \
       this browser can't do — and with encryption on it can't be sent to \
       the server. Import it with the hygg desktop or CLI client."
        .to_string(),
    );
  }

  match sync::convert(&creds, &name, &bytes, col).await {
    Ok(resp) => {
      let book =
        book_from_server_text(&resp.title, &resp.format, &resp.text, &bytes);
      finish(&book, &bytes, &Some(creds), scope).await
    }
    Err(ConvertErr::Denied(body)) => ImportResult::Denied(body),
    Err(ConvertErr::Failed(e)) => ImportResult::Message(e),
  }
}

/// Persist a built book locally and upload it when connected. The upload honors
/// the auto-sync scope (a document the scope doesn't cover stays local until
/// opted in) and the document's effective sync mode: `full` sends the bytes,
/// `metadata` only registers the record, `off` uploads nothing.
async fn finish(
  book: &Book,
  bytes: &[u8],
  creds: &Creds,
  scope: hygg_shared::sync::AutoSyncPolicy,
) -> ImportResult {
  if storage::put_book(book, bytes).await.is_err() {
    return ImportResult::Message("Couldn't save the document.".to_string());
  }
  if let Some(creds) = creds {
    upload_book_if_synced(creds, &book.id, scope).await;
  }
  ImportResult::Saved
}

/// Upload a stored book to the server when the auto-sync scope covers it and
/// its effective sync mode permits — `full` sends the bytes, `metadata`
/// registers the record only, `off`/uncovered uploads nothing. Used on import
/// and when a document is opted into auto-sync.
pub async fn upload_book_if_synced(
  creds: &sync::Creds,
  id: &str,
  scope: hygg_shared::sync::AutoSyncPolicy,
) {
  let Some(summary) = storage::get_summary(id).await else {
    return;
  };
  if !summary.auto_syncs(scope) {
    return;
  }
  let mode = summary.effective_sync_mode();
  if mode.syncs_blob() {
    if let Some(bytes) = storage::get_blob(id).await {
      let _ =
        sync::upload_book(creds, id, &summary.title, &summary.format, &bytes)
          .await;
    }
  } else if mode.syncs_state() {
    let _ = sync::upload_book_meta(
      creds,
      id,
      &summary.title,
      &summary.format,
      summary.size_bytes as i64,
    )
    .await;
  }
}

/// Turn a *downloaded* document's bytes into an openable book: client-side
/// extraction when the browser can handle the format, else the server's
/// canonical extraction of the same stored document (DOCX, scanned PDFs the
/// browser can't extract offline). The server reads its retained blob, so the
/// downloaded bytes are not re-uploaded. A server that declines to convert
/// yields [`DownloadError::Denied`] carrying its own words; other failures are
/// [`DownloadError::Unavailable`].
pub async fn book_from_download(
  creds: &sync::Creds,
  id: &str,
  filename: &str,
  bytes: &[u8],
  col: usize,
) -> Result<Book, DownloadError> {
  if let Ok(book) = import(filename, bytes, col).await
    && has_text(&book)
  {
    return Ok(book);
  }
  match sync::fetch_extraction(creds, id, col).await {
    Ok(resp) => {
      Ok(book_from_server_text(&resp.title, &resp.format, &resp.text, bytes))
    }
    Err(ConvertErr::Denied(body)) => Err(DownloadError::Denied(body)),
    Err(ConvertErr::Failed(e)) => Err(DownloadError::Unavailable(e)),
  }
}

/// At least a few non-empty lines — a quick "did extraction work" check.
fn has_text(book: &Book) -> bool {
  book.lines.iter().filter(|l| !l.trim().is_empty()).count() >= 3
}
