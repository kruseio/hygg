//! Offline document storage on IndexedDB (via `rexie`).
//!
//! Four object stores, all keyed by `book_id` (sha256 hex of the source bytes):
//! - `library`  — small [`BookSummary`] rows for the Home grid (cheap to list);
//! - `books`    — the full [`Book`] (every rendered line) for the reader;
//! - `progress` — per-book reading position;
//! - `blobs`    — the original source bytes, retained for later server sync.
//!
//! Each call opens a fresh connection; IndexedDB open is cheap and this keeps
//! the surface free of shared mutable handles. Records are stored as JSON
//! strings (serde) except blobs, which are raw `Uint8Array`s.

use js_sys::Uint8Array;
use rexie::{ObjectStore, Rexie, TransactionMode};
use wasm_bindgen::JsValue;

use crate::model::{Book, BookSummary, Progress};

type Res<T> = Result<T, Box<dyn std::error::Error>>;

const DB: &str = "hygg";
const LIBRARY: &str = "library";
const BOOKS: &str = "books";
const PROGRESS: &str = "progress";
const BLOBS: &str = "blobs";

async fn open() -> Res<Rexie> {
  Rexie::builder(DB)
    .version(1)
    .add_object_store(ObjectStore::new(LIBRARY))
    .add_object_store(ObjectStore::new(BOOKS))
    .add_object_store(ObjectStore::new(PROGRESS))
    .add_object_store(ObjectStore::new(BLOBS))
    .build()
    .await
    .map_err(Into::into)
}

fn key(id: &str) -> JsValue {
  JsValue::from_str(id)
}

/// Persist a freshly-imported book: its summary, full lines, and source bytes.
pub async fn put_book(book: &Book, source_bytes: &[u8]) -> Res<()> {
  let db = open().await?;
  let tx =
    db.transaction(&[LIBRARY, BOOKS, BLOBS], TransactionMode::ReadWrite)?;

  // Carry any prior per-document sync settings (server ceiling + local clamp)
  // forward: a fresh `BookSummary::from` would otherwise reset them when a
  // metadata-only row is upgraded to a full book.
  let mut summary_row = BookSummary::from(book);
  if let Some(existing) = tx
    .store(LIBRARY)?
    .get(key(&book.id))
    .await?
    .and_then(|v| v.as_string())
    .and_then(|s| serde_json::from_str::<BookSummary>(&s).ok())
  {
    summary_row.sync_mode = existing.sync_mode;
    summary_row.local_sync_mode = existing.local_sync_mode;
    summary_row.auto_sync_optin = existing.auto_sync_optin;
  }
  let summary = JsValue::from_str(&serde_json::to_string(&summary_row)?);
  let full = JsValue::from_str(&serde_json::to_string(book)?);
  let bytes: JsValue = Uint8Array::from(source_bytes).into();
  let k = key(&book.id);

  tx.store(LIBRARY)?.put(&summary, Some(&k)).await?;
  tx.store(BOOKS)?.put(&full, Some(&k)).await?;
  tx.store(BLOBS)?.put(&bytes, Some(&k)).await?;

  tx.done().await?;
  Ok(())
}

/// Persist just a book's library summary (metadata), without its content — so a
/// server document appears on the home before its bytes finish downloading. A
/// later `put_book` overwrites this row with the full summary (real line
/// count).
pub async fn put_summary(summary: &BookSummary) -> Res<()> {
  let db = open().await?;
  let tx = db.transaction(&[LIBRARY], TransactionMode::ReadWrite)?;
  let value = JsValue::from_str(&serde_json::to_string(summary)?);
  tx.store(LIBRARY)?.put(&value, Some(&key(&summary.id))).await?;
  tx.done().await?;
  Ok(())
}

/// Mutate a stored library summary in place and persist it, keeping every other
/// field. No-op when the book has no summary row yet. Used to update per-book
/// sync settings without disturbing the rest of the record.
pub async fn update_summary(
  id: &str,
  f: impl FnOnce(&mut BookSummary),
) -> Res<()> {
  if let Some(mut summary) = get_summary(id).await {
    f(&mut summary);
    put_summary(&summary).await?;
  }
  Ok(())
}

/// Set this device's local sync preference for a document (`None` = inherit the
/// account-wide ceiling).
pub async fn set_local_sync_mode(
  id: &str,
  mode: Option<hygg_shared::sync::SyncMode>,
) -> Res<()> {
  update_summary(id, |s| s.local_sync_mode = mode).await
}

/// Set this device's explicit "auto-sync this document" opt-in.
pub async fn set_auto_sync_optin(id: &str, opt_in: bool) -> Res<()> {
  update_summary(id, |s| s.auto_sync_optin = opt_in).await
}

/// A single library summary (metadata) by id, if present.
pub async fn get_summary(id: &str) -> Option<BookSummary> {
  let db = open().await.ok()?;
  let tx = db.transaction(&[LIBRARY], TransactionMode::ReadOnly).ok()?;
  let value = tx.store(LIBRARY).ok()?.get(key(id)).await.ok()?;
  let _ = tx.done().await;
  value.and_then(|v| v.as_string()).and_then(|s| serde_json::from_str(&s).ok())
}

/// Whether a book's full content (rendered lines) is stored locally — false for
/// metadata-only rows whose bytes a background sync hasn't fetched yet.
pub async fn has_book(id: &str) -> bool {
  async {
    let db = open().await?;
    let tx = db.transaction(&[BOOKS], TransactionMode::ReadOnly)?;
    let value = tx.store(BOOKS)?.get(key(id)).await?;
    tx.done().await?;
    Ok::<bool, Box<dyn std::error::Error>>(
      value.and_then(|v| v.as_string()).is_some(),
    )
  }
  .await
  .unwrap_or(false)
}

/// All library summaries, newest-imported first.
pub async fn list_library() -> Res<Vec<BookSummary>> {
  let db = open().await?;
  let tx = db.transaction(&[LIBRARY], TransactionMode::ReadOnly)?;
  let rows = tx.store(LIBRARY)?.get_all(None, None).await?;
  tx.done().await?;

  let mut out: Vec<BookSummary> = rows
    .into_iter()
    .filter_map(|v| v.as_string())
    .filter_map(|s| serde_json::from_str(&s).ok())
    .collect();
  out.sort_by(|a, b| {
    b.added_at.partial_cmp(&a.added_at).unwrap_or(std::cmp::Ordering::Equal)
  });
  Ok(out)
}

/// Load a full book (every rendered line) for the reader.
pub async fn get_book(id: &str) -> Res<Option<Book>> {
  let db = open().await?;
  let tx = db.transaction(&[BOOKS], TransactionMode::ReadOnly)?;
  let value = tx.store(BOOKS)?.get(key(id)).await?;
  tx.done().await?;
  Ok(
    value
      .and_then(|v| v.as_string())
      .and_then(|s| serde_json::from_str(&s).ok()),
  )
}

/// The original source bytes retained in the `blobs` store, if present. Used to
/// re-extract a locally-cached document (e.g. to upgrade a book imported before
/// page tracking existed) without a network round-trip.
pub async fn get_blob(id: &str) -> Option<Vec<u8>> {
  let db = open().await.ok()?;
  let tx = db.transaction(&[BLOBS], TransactionMode::ReadOnly).ok()?;
  let value = tx.store(BLOBS).ok()?.get(key(id)).await.ok()??;
  let _ = tx.done().await;
  Some(Uint8Array::new(&value).to_vec())
}

/// Remove a book and all its associated rows.
pub async fn delete_book(id: &str) -> Res<()> {
  let db = open().await?;
  let tx = db.transaction(
    &[LIBRARY, BOOKS, PROGRESS, BLOBS],
    TransactionMode::ReadWrite,
  )?;
  for store in [LIBRARY, BOOKS, PROGRESS, BLOBS] {
    tx.store(store)?.delete(key(id)).await?;
  }
  tx.done().await?;
  Ok(())
}

/// Reading position for a book (defaults to the start if never opened).
pub async fn get_progress(id: &str) -> Res<Progress> {
  let db = open().await?;
  let tx = db.transaction(&[PROGRESS], TransactionMode::ReadOnly)?;
  let value = tx.store(PROGRESS)?.get(key(id)).await?;
  tx.done().await?;
  Ok(
    value
      .and_then(|v| v.as_string())
      .and_then(|s| serde_json::from_str(&s).ok())
      .unwrap_or_default(),
  )
}

/// Save reading position. Best-effort; callers fire-and-forget.
pub async fn put_progress(id: &str, progress: Progress) -> Res<()> {
  let db = open().await?;
  let tx = db.transaction(&[PROGRESS], TransactionMode::ReadWrite)?;
  let value = JsValue::from_str(&serde_json::to_string(&progress)?);
  tx.store(PROGRESS)?.put(&value, Some(&key(id))).await?;
  tx.done().await?;
  Ok(())
}

// Note: the original source bytes are written to the `blobs` store by
// `put_book` so imported documents are already sync-ready; the reader for them
// (`get_blob`) lands with server upload in Phase 3.
