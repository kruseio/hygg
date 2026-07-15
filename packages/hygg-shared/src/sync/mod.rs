//! Types and helpers shared between the hygg client and the sync server.
//!
//! - [`identity`]: the content-derived book identity (a stable cross-device
//!   id).
//! - [`mode`]: the per-document sync policy (full / metadata / off).
//! - [`autosync`]: the account/device-wide automatic-sync scope (which
//!   documents sync at all) plus the "looks like a book" heuristic.
//! - [`proto`]: the JSON wire contract for the device + sync API, so the AGPL
//!   client and the Elastic-licensed server stay statically typed against one
//!   definition without either license crossing the boundary.

pub mod autosync;
pub mod clock;
pub mod headers;
pub mod identity;
pub mod mode;
pub mod proto;

pub use autosync::{
  AutoSyncPolicy, BOOK_MIN_PDF_PAGES, BOOK_MIN_TEXT_LINES, looks_like_book,
  should_auto_sync,
};
pub use identity::{book_id_for_file, book_id_from_text, content_sha256};
pub use mode::SyncMode;
