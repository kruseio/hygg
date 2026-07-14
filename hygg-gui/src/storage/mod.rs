//! Offline document store: a per-user data directory of JSON files.
//!
//! Four logical stores, all keyed by `book_id` (sha256 hex of the source
//! bytes): `library` (small [`BookSummary`] rows for the Home grid), `books`
//! (the full [`Book`]), `progress` (per-book position), and `blobs` (original
//! source bytes, retained for re-extraction and later server sync).

use hygg_shared::sync::SyncMode;

use crate::model::BookSummary;

mod native;
pub use native::*;

/// Set this device's local sync preference for a document (`None` = inherit the
/// account-wide ceiling). Implemented on top of the backend's summary getter/
/// setter so both platforms share the merge logic.
pub async fn set_local_sync_mode(
  id: String,
  mode: Option<SyncMode>,
) -> Result<(), String> {
  if let Some(mut summary) = get_summary(id.clone()).await {
    summary.local_sync_mode = mode;
    put_summary(summary).await?;
  }
  Ok(())
}

/// Set this device's explicit "auto-sync this document" opt-in.
pub async fn set_auto_sync_optin(
  id: String,
  opt_in: bool,
) -> Result<(), String> {
  if let Some(mut summary) = get_summary(id.clone()).await {
    summary.auto_sync_optin = opt_in;
    put_summary(summary).await?;
  }
  Ok(())
}

/// Carry any prior per-document sync settings forward onto a freshly built
/// summary, so upgrading a metadata-only row to a full book doesn't reset them.
pub(crate) fn merge_sync_settings(
  fresh: &mut BookSummary,
  existing: Option<&BookSummary>,
) {
  if let Some(existing) = existing {
    fresh.sync_mode = existing.sync_mode;
    fresh.local_sync_mode = existing.local_sync_mode;
    fresh.auto_sync_optin = existing.auto_sync_optin;
  }
}
