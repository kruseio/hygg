//! `SyncClient` request methods (upload/meta/library/push/pull). Split out from
//! the client module to keep each file within the repository's per-file line
//! budget; behaviour and the wire contract are unchanged.

use hygg_shared::sync::headers::{MACHINE_ID_HEADER, USER_HEADER};
use hygg_shared::sync::proto;

use super::super::inbound::RemoteBook;
use super::super::types::ProgressPayload;
use super::{PullResult, SyncClient, UploadError};

/// Classify a failed request: a 4xx (except 408 request-timeout and 429
/// too-many-requests) is permanent; everything else is worth retrying.
fn upload_error(err: ureq::Error) -> UploadError {
  let permanent = matches!(
    err,
    ureq::Error::StatusCode(code)
      if (400..500).contains(&code) && code != 408 && code != 429
  );
  UploadError { permanent, message: err.to_string() }
}

impl SyncClient {
  /// Register (or refresh) a document's metadata record without its bytes. Used
  /// on its own for metadata-only sync, and as the first step of a full upload.
  fn upsert_meta(
    &self,
    content_hash: &str,
    title: &str,
    format: &str,
    size_bytes: i64,
  ) -> Result<(), UploadError> {
    let meta_url = format!("{}/api/v1/books", self.base_url);
    let meta = proto::UpsertBookRequest {
      content_hash: content_hash.to_string(),
      title: title.to_string(),
      author: String::new(),
      format: format.to_string(),
      size_bytes,
      // Never touch the account-wide ceiling from a routine upload; the ceiling
      // is set explicitly via `set_book_sync_mode` / the web admin.
      sync_mode: None,
    };
    self
      .agent
      .post(&meta_url)
      .header("Authorization", &self.bearer())
      .header(USER_HEADER, &self.username)
      .header(MACHINE_ID_HEADER, &self.machine_id)
      .send_json(&meta)
      .map_err(upload_error)?;
    Ok(())
  }

  /// Upload a book: register its metadata, then upload the document bytes
  /// (keyed by `content_hash`, the cross-device book id).
  pub fn upload_book(
    &self,
    content_hash: &str,
    title: &str,
    format: &str,
    bytes: &[u8],
  ) -> Result<(), UploadError> {
    self.upsert_meta(content_hash, title, format, bytes.len() as i64)?;
    let blob_url =
      format!("{}/api/v1/books/{content_hash}/blob", self.base_url);
    self
      .agent
      .put(&blob_url)
      .header("Authorization", &self.bearer())
      .header(USER_HEADER, &self.username)
      .header(MACHINE_ID_HEADER, &self.machine_id)
      .header("Content-Type", "application/octet-stream")
      .send(bytes)
      .map_err(upload_error)?;
    Ok(())
  }

  /// Register a book's metadata record only — the file bytes stay on this
  /// device. The metadata-only sync path.
  pub fn upload_book_meta(
    &self,
    content_hash: &str,
    title: &str,
    format: &str,
    size_bytes: i64,
  ) -> Result<(), UploadError> {
    self.upsert_meta(content_hash, title, format, size_bytes)
  }

  /// Set the account-wide sync ceiling for a document. Authoritative for every
  /// device on the account; each clamps its local preference against it.
  pub fn set_book_sync_mode(
    &self,
    content_hash: &str,
    mode: proto::SyncMode,
  ) -> Result<(), String> {
    let url =
      format!("{}/api/v1/books/{content_hash}/sync-mode", self.base_url);
    self
      .agent
      .put(&url)
      .header("Authorization", &self.bearer())
      .header(USER_HEADER, &self.username)
      .header(MACHINE_ID_HEADER, &self.machine_id)
      .send_json(&proto::SetSyncModeRequest { sync_mode: mode })
      .map_err(|e| e.to_string())?;
    Ok(())
  }

  /// List the books available to this device on the server.
  pub fn fetch_library(&self) -> Result<Vec<RemoteBook>, String> {
    let url = format!("{}/api/v1/books", self.base_url);
    let books: Vec<proto::BookDto> = self
      .agent
      .get(&url)
      .header("Authorization", &self.bearer())
      .header(USER_HEADER, &self.username)
      .header(MACHINE_ID_HEADER, &self.machine_id)
      .call()
      .map_err(|e| e.to_string())?
      .body_mut()
      .read_json()
      .map_err(|e| e.to_string())?;
    Ok(books.into_iter().map(RemoteBook::from).collect())
  }

  /// Download a book's document bytes by its `content_hash`.
  pub fn download_book(&self, content_hash: &str) -> Result<Vec<u8>, String> {
    let url = format!("{}/api/v1/books/{content_hash}/blob", self.base_url);
    let mut response = self
      .agent
      .get(&url)
      .header("Authorization", &self.bearer())
      .header(USER_HEADER, &self.username)
      .header(MACHINE_ID_HEADER, &self.machine_id)
      .call()
      .map_err(|e| e.to_string())?;
    response
      .body_mut()
      .with_config()
      .limit(u64::MAX)
      .read_to_vec()
      .map_err(|e| e.to_string())
  }

  /// Push a batch of typed ops (progress and/or annotations) in one request —
  /// the anti-spam guarantee is one push per engine cycle.
  pub fn push(&self, ops: &[proto::SyncOp]) -> Result<(), String> {
    if ops.is_empty() {
      return Ok(());
    }
    let url = format!("{}/api/v1/sync/push", self.base_url);
    let request = proto::PushRequest { device_id: None, ops: ops.to_vec() };
    self
      .agent
      .post(&url)
      .header("Authorization", &self.bearer())
      .header(USER_HEADER, &self.username)
      .header(MACHINE_ID_HEADER, &self.machine_id)
      .send_json(&request)
      .map_err(|e| e.to_string())?;
    Ok(())
  }

  pub fn push_progress(&self, items: &[ProgressPayload]) -> Result<(), String> {
    let ops: Vec<proto::SyncOp> =
      items.iter().map(ProgressPayload::to_op).collect();
    self.push(&ops)
  }

  /// Pull everything changed since `since` (progress + annotations), converting
  /// the shared pull DTOs into the editor-facing types.
  pub fn pull(&self, since: i64) -> Result<PullResult, String> {
    let url = format!("{}/api/v1/sync/pull", self.base_url);
    let response: proto::PullResponse = self
      .agent
      .get(&url)
      .query("since", since.to_string())
      .header("Authorization", &self.bearer())
      .header(USER_HEADER, &self.username)
      .header(MACHINE_ID_HEADER, &self.machine_id)
      .call()
      .map_err(|e| e.to_string())?
      .body_mut()
      .read_json()
      .map_err(|e| e.to_string())?;
    Ok(PullResult {
      server_time: response.server_time,
      progress: response.progress.into_iter().map(Into::into).collect(),
      bookmarks: response.bookmarks.into_iter().map(Into::into).collect(),
      highlights: response.highlights.into_iter().map(Into::into).collect(),
      notes: response.notes.into_iter().map(Into::into).collect(),
    })
  }
}
