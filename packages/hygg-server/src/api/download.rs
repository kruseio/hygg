//! Response shaping for raw document downloads.
//!
//! hygg-server is a sync backend, not a reader: it hands document bytes to a
//! client to render and never renders a document itself. These headers make
//! that explicit at the HTTP layer, so the server cannot be turned into a
//! reader even by pointing a browser straight at a blob URL:
//!
//! - `Content-Disposition: attachment` — a browser downloads the bytes, it
//!   never displays them inline as a page.
//! - `X-Content-Type-Options: nosniff` — the declared
//!   `application/octet-stream` is honoured, so an HTML or PDF blob can't be
//!   MIME-sniffed into an inline render.
//!
//! Programmatic sync clients (the CLI's `ureq`, the PWA's `fetch`) read the
//! body regardless of these headers, so nothing about sync changes.

use axum::http::header;
use axum::response::{IntoResponse, Response};

/// Serve document bytes as a plain download that is never rendered inline.
pub fn document_download(bytes: Vec<u8>) -> Response {
  (
    [
      (header::CONTENT_TYPE, "application/octet-stream"),
      (header::CONTENT_DISPOSITION, "attachment"),
      (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
    ],
    bytes,
  )
    .into_response()
}
