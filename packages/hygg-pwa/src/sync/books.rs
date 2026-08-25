//! Document storage and server-side conversion over the `/api/v1` books API:
//! metadata upsert, blob upload/download, listing, and the gated
//! conversion/extraction endpoints. Split out of [`super`] to keep each module
//! within the LOC budget; shares the auth helpers via `super::`.

use gloo_net::http::Request;
use hygg_shared::sync::proto::{BookDto, DenialBody, UpsertBookRequest};
use js_sys::Uint8Array;

use super::{Creds, Res, api, authed, error_body};

/// Read a 403's body as the server's explanation. A server that refuses
/// without one still produces a usable refusal, just with nothing to show
/// beyond the bare status.
async fn denial(resp: gloo_net::http::Response) -> ConvertErr {
  match resp.json::<DenialBody>().await {
    Ok(body) => ConvertErr::Denied(body),
    Err(_) => ConvertErr::Denied(DenialBody {
      error: "The server declined this request.".into(),
      action_url: None,
      action_label: None,
    }),
  }
}

/// Register (or refresh) a document's metadata record without its bytes. Used
/// on its own for metadata-only sync, and as the first step of a full upload.
async fn upsert_meta(
  creds: &Creds,
  book_id: &str,
  title: &str,
  format: &str,
  size_bytes: i64,
) -> Res<()> {
  let meta = UpsertBookRequest {
    content_hash: book_id.to_string(),
    title: title.to_string(),
    author: String::new(),
    format: format.to_string(),
    size_bytes,
    // The ceiling is set explicitly via `set_book_sync_mode`, never clobbered
    // by a routine metadata push.
    sync_mode: None,
  };
  let resp = authed(Request::post(&api(&creds.server, "/books")), creds)
    .json(&meta)
    .map_err(|e| e.to_string())?
    .send()
    .await
    .map_err(|e| e.to_string())?;
  if resp.ok() { Ok(()) } else { Err(error_body(resp).await) }
}

/// Register a document's metadata record only — the source bytes stay on this
/// device. The metadata-only sync path.
pub async fn upload_book_meta(
  creds: &Creds,
  book_id: &str,
  title: &str,
  format: &str,
  size_bytes: i64,
) -> Res<()> {
  upsert_meta(creds, book_id, title, format, size_bytes).await
}

/// Upload a document: metadata then raw bytes. Idempotent by content hash.
pub async fn upload_book(
  creds: &Creds,
  book_id: &str,
  title: &str,
  format: &str,
  bytes: &[u8],
) -> Res<()> {
  // Size metadata reflects the plaintext (stable across encryption on/off);
  // the payload is the sealed envelope when a key is set up on this browser.
  upsert_meta(creds, book_id, title, format, bytes.len() as i64).await?;
  let payload = match &creds.key {
    Some(key) => {
      hygg_shared::crypto::encrypt(key, bytes).map_err(|e| e.to_string())?
    }
    None => bytes.to_vec(),
  };
  let blob = Uint8Array::from(payload.as_slice());
  let resp = authed(
    Request::put(&api(&creds.server, &format!("/books/{book_id}/blob"))),
    creds,
  )
  .header("Content-Type", "application/octet-stream")
  .body(blob)
  .map_err(|e| e.to_string())?
  .send()
  .await
  .map_err(|e| e.to_string())?;
  if resp.ok() { Ok(()) } else { Err(error_body(resp).await) }
}

/// List documents the connected account can read.
pub async fn list_books(creds: &Creds) -> Res<Vec<BookDto>> {
  let resp = authed(Request::get(&api(&creds.server, "/books")), creds)
    .send()
    .await
    .map_err(|e| e.to_string())?;
  if !resp.ok() {
    return Err(error_body(resp).await);
  }
  resp.json().await.map_err(|e| e.to_string())
}

/// Download a document's raw bytes by content hash.
pub async fn download_blob(creds: &Creds, book_id: &str) -> Res<Vec<u8>> {
  let resp = authed(
    Request::get(&api(&creds.server, &format!("/books/{book_id}/blob"))),
    creds,
  )
  .send()
  .await
  .map_err(|e| e.to_string())?;
  if !resp.ok() {
    return Err(error_body(resp).await);
  }
  let bytes = resp.binary().await.map_err(|e| e.to_string())?;
  // Decrypt a sealed blob when this browser holds the key; a plaintext blob
  // (encryption off, or a document not yet converted) passes through. An
  // envelope with no key is unreadable here.
  if hygg_shared::crypto::is_envelope(&bytes) {
    return match &creds.key {
      Some(key) => {
        hygg_shared::crypto::decrypt(key, &bytes).map_err(|e| e.to_string())
      }
      None => Err(
        "this document is encrypted but no key is set up in this browser"
          .to_string(),
      ),
    };
  }
  Ok(bytes)
}

/// Server-side conversion response for a format the browser can't handle.
#[derive(serde::Deserialize)]
pub struct ConvertResp {
  pub title: String,
  pub format: String,
  pub text: String,
}

/// Why a server conversion failed. A 403 is kept apart because the server
/// hands back wording (and sometimes a link) that the UI shows as-is.
pub enum ConvertErr {
  Denied(DenialBody),
  Failed(String),
}

/// Convert an upload server-side (scanned-PDF OCR / pandoc formats) and get
/// back justified text. A server that declines yields [`ConvertErr::Denied`]
/// carrying its own explanation.
pub async fn convert(
  creds: &Creds,
  filename: &str,
  bytes: &[u8],
  col: usize,
) -> Result<ConvertResp, ConvertErr> {
  let enc = String::from(js_sys::encode_uri_component(filename));
  let url = format!(
    "{}/api/v1/convert?filename={enc}&col={col}",
    creds.server.trim_end_matches('/')
  );
  let resp = authed(Request::post(&url), creds)
    .header("Content-Type", "application/octet-stream")
    .body(Uint8Array::from(bytes))
    .map_err(|e| ConvertErr::Failed(e.to_string()))?
    .send()
    .await
    .map_err(|e| ConvertErr::Failed(e.to_string()))?;
  if resp.status() == 403 {
    return Err(denial(resp).await);
  }
  if !resp.ok() {
    return Err(ConvertErr::Failed(error_body(resp).await));
  }
  resp.json().await.map_err(|e| ConvertErr::Failed(e.to_string()))
}

/// Fetch the server's canonical extraction of an already-stored document (by
/// content hash), for a format the browser can't extract itself. Unlike
/// [`convert`], the bytes are not re-uploaded — the server reads its retained
/// blob — so this is cheap for large scanned PDFs. Same gate.
pub async fn fetch_extraction(
  creds: &Creds,
  book_id: &str,
  col: usize,
) -> Result<ConvertResp, ConvertErr> {
  let url = format!(
    "{}/api/v1/books/{book_id}/extraction?col={col}",
    creds.server.trim_end_matches('/')
  );
  let resp = authed(Request::get(&url), creds)
    .send()
    .await
    .map_err(|e| ConvertErr::Failed(e.to_string()))?;
  if resp.status() == 403 {
    return Err(denial(resp).await);
  }
  if !resp.ok() {
    return Err(ConvertErr::Failed(error_body(resp).await));
  }
  resp.json().await.map_err(|e| ConvertErr::Failed(e.to_string()))
}
