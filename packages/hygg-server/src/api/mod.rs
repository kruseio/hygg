//! The JSON sync/device API, mounted under `/api/v1`.

pub mod books;
pub mod convert;
pub mod devices;
pub mod events;
pub mod export;
mod export_inputs;
pub mod extraction;
mod pandoc;
pub mod sync;
mod sync_inputs;
// Row/principal -> shared-DTO conversions; no exported names, just trait impls.
mod dto;

use axum::Router;
use axum::routing::{delete, get, post, put};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
  Router::new()
    .route("/api/v1/devices/register", post(devices::register))
    .route("/api/v1/devices", get(devices::list_devices))
    .route("/api/v1/devices/{id}", delete(devices::revoke_device))
    .route("/api/v1/me", get(devices::me))
    .route("/api/v1/sync/push", post(sync::push))
    .route("/api/v1/sync/pull", get(sync::pull))
    .route("/api/v1/events", get(events::events))
    .route("/api/v1/books", get(books::list_books).post(books::upsert_book))
    .route(
      "/api/v1/books/{content_hash}/blob",
      put(books::put_blob).get(books::get_blob),
    )
    .route(
      "/api/v1/books/{content_hash}/sync-mode",
      put(books::set_book_sync_mode),
    )
    .route(
      "/api/v1/books/{content_hash}/extraction",
      get(extraction::get_extraction),
    )
    .route("/api/v1/convert", post(convert::convert))
    .route("/api/v1/export", get(export::export))
    .route("/api/v1/import", post(export::import))
}
