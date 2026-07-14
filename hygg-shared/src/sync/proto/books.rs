//! Document (book) metadata shapes.

use serde::{Deserialize, Serialize};

use crate::sync::mode::SyncMode;

/// `POST /api/v1/books` request body: register or update a document's metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertBookRequest {
  pub content_hash: String,
  #[serde(default)]
  pub title: String,
  #[serde(default)]
  pub author: String,
  #[serde(default)]
  pub format: String,
  #[serde(default)]
  pub size_bytes: i64,
  /// When `Some`, also set the account-wide sync ceiling for this document.
  /// `None` (the common case) leaves any existing policy untouched, so a
  /// routine metadata refresh never resets the mode.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub sync_mode: Option<SyncMode>,
}

/// `PUT /api/v1/books/{content_hash}/sync-mode` request body: set the
/// account-wide sync ceiling for a document. Clients read the current value
/// from [`BookDto::sync_mode`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetSyncModeRequest {
  pub sync_mode: SyncMode,
}

/// `POST /api/v1/books` response body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertBookResponse {
  pub content_hash: String,
}

/// One row of `GET /api/v1/books`: a document available to the caller.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BookDto {
  pub content_hash: String,
  pub title: String,
  pub author: String,
  pub format: String,
  pub size_bytes: i64,
  pub updated_at: i64,
  /// The account-wide sync ceiling for this document. Clients clamp their own
  /// (equal-or-more-restrictive) local preference against it. `serde(default)`
  /// keeps rows written before this field existed decoding as
  /// [`SyncMode::Full`].
  #[serde(default)]
  pub sync_mode: SyncMode,
}

/// `PUT /api/v1/books/{content_hash}/blob` response body. (The request and the
/// `GET` response are raw document bytes — the only non-DTO payloads, since a
/// blob has no structure to type.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PutBlobResponse {
  pub byte_len: u64,
  pub sha256: String,
}
