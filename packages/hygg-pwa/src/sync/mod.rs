//! Optional server sync client (progressive enhancement). Talks to the
//! bearer-token `/api/v1` JSON API using the shared `hygg-shared` wire DTOs, so
//! a document synced here lines up with its CLI/other-device twin. Everything
//! here is best-effort: failures are returned as strings and surfaced quietly —
//! the reader never depends on the network.
//!
//! Auth + progress live here; document storage (upload/download) and
//! server-side conversion live in [`books`], split out to keep each module
//! within the LOC budget.

use gloo_net::http::{Request, RequestBuilder};
use hygg_shared::sync::headers::{MACHINE_ID_HEADER, USER_HEADER};
use hygg_shared::sync::proto::{
  MeResponse, OpPayload, ProgressData, ProgressDto, PullResponse, PushRequest,
  PushResponse, RegisterDeviceRequest, RegisterDeviceResponse, SignupRequest,
  SignupResponse, SyncOp,
};
use uuid::Uuid;

mod books;
mod commerce;
mod encryption;
pub use books::{
  ConvertErr, ConvertResp, convert, download_blob, fetch_extraction,
  list_books, upload_book, upload_book_meta,
};
pub use commerce::{fetch_plans, start_checkout};
pub use encryption::{
  convert_library, disable_encryption, enable_encryption, get_encryption,
};

/// How this browser names itself to the server when it registers a device.
const DEVICE_NAME: &str = "hygg PWA";
const PLATFORM: &str = "web";

type Res<T> = Result<T, String>;

/// Everything a request needs to authenticate: the server plus the three-part
/// credential the API now requires — the bearer token, the account username,
/// and this browser's machine id (which the token is bound to). Also carries
/// the account content key when encryption is set up, so the blob paths can
/// seal/open document bytes at the request boundary.
#[derive(Clone, Debug)]
pub struct Creds {
  pub server: String,
  pub token: String,
  pub username: String,
  pub machine_id: String,
  /// The derived content key when this browser is set up for encryption;
  /// `None` leaves uploads/downloads in the clear (encryption off).
  pub key: Option<hygg_shared::crypto::EncryptionKey>,
}

fn api(server: &str, path: &str) -> String {
  format!("{}/api/v1{}", server.trim_end_matches('/'), path)
}

/// Timestamp for an outbound op, in the server's clock domain (skew-corrected)
/// so last-write-wins orders this device's writes correctly against peers.
fn now_ms() -> i64 {
  crate::clock::now_ms() as i64
}

/// Attach the full auth triple (token + username + machine id) to a request.
fn authed(req: RequestBuilder, creds: &Creds) -> RequestBuilder {
  req
    .header("Authorization", &format!("Bearer {}", creds.token))
    .header(USER_HEADER, &creds.username)
    .header(MACHINE_ID_HEADER, &creds.machine_id)
}

/// Identify the authenticated principal — validates the username + device
/// token (and binds this browser's machine id on first use), yielding the
/// device id + account label (the PWA connects by username + token, like the
/// CLI's `:auth`).
pub async fn fetch_me(creds: &Creds) -> Res<MeResponse> {
  let resp = authed(Request::get(&api(&creds.server, "/me")), creds)
    .send()
    .await
    .map_err(|e| e.to_string())?;
  if !resp.ok() {
    return Err(error_body(resp).await);
  }
  resp.json().await.map_err(|e| e.to_string())
}

/// POST a JSON body to an *unauthenticated* endpoint (signup / registration,
/// which exchange credentials for a token) and parse the JSON reply.
async fn post_json<B, T>(url: &str, body: &B) -> Res<T>
where
  B: serde::Serialize,
  T: serde::de::DeserializeOwned,
{
  let resp = Request::post(url)
    .json(body)
    .map_err(|e| e.to_string())?
    .send()
    .await
    .map_err(|e| e.to_string())?;
  if !resp.ok() {
    return Err(error_body(resp).await);
  }
  resp.json().await.map_err(|e| e.to_string())
}

/// Create an account and receive its first device token in one call
/// (`POST /api/v1/signup`). The password is sent once to mint the token and is
/// never stored — only the returned token is kept. `machine_id` binds the new
/// device to this browser.
pub async fn signup(
  server: &str,
  email: &str,
  password: &str,
  machine_id: &str,
) -> Res<SignupResponse> {
  let body = SignupRequest {
    email: email.to_string(),
    password: password.to_string(),
    display_name: String::new(),
    device_name: DEVICE_NAME.to_string(),
    platform: PLATFORM.to_string(),
    machine_id: Some(machine_id.to_string()),
  };
  post_json(&api(server, "/signup"), &body).await
}

/// Register a new device for an *existing* account
/// (`POST /api/v1/devices/register`), exchanging email + password for this
/// browser's own device token. Same one-time-password, token-only-stored model
/// as [`signup`].
pub async fn register_device(
  server: &str,
  email: &str,
  password: &str,
  machine_id: &str,
) -> Res<RegisterDeviceResponse> {
  let body = RegisterDeviceRequest {
    email: email.to_string(),
    password: password.to_string(),
    device_name: DEVICE_NAME.to_string(),
    platform: PLATFORM.to_string(),
    machine_id: Some(machine_id.to_string()),
  };
  post_json(&api(server, "/devices/register"), &body).await
}

/// Push the latest reading position for a book (last-write-wins by time).
#[allow(clippy::too_many_arguments)]
pub async fn push_progress(
  creds: &Creds,
  device_id: &str,
  book_id: &str,
  offset: u64,
  total_lines: u64,
  percentage: f64,
  // Pagination-independent PDF anchor (1-based page + line within it) so a
  // reader that wraps the document at a different width resumes on the same
  // page. `None` for reflowable formats, which sync by percentage.
  page: Option<u32>,
  line_in_page: Option<u64>,
  // Exact resume anchor: non-whitespace character offset of the center line
  // (page-local for PDFs, global otherwise) — the same content resolves to the
  // same anchor in any reader, at any width. See `hygg_shared::anchor`.
  word_offset: Option<u64>,
) -> Res<()> {
  let op = SyncOp {
    op_id: Uuid::new_v4().to_string(),
    book_id: book_id.to_string(),
    deleted: false,
    updated_at: now_ms(),
    payload: OpPayload::Progress(ProgressData {
      offset,
      total_lines,
      percentage,
      viewport_offset: None,
      cursor_y: None,
      page,
      line_in_page,
      word_offset,
    }),
  };
  let body =
    PushRequest { device_id: Some(device_id.to_string()), ops: vec![op] };
  let resp = authed(Request::post(&api(&creds.server, "/sync/push")), creds)
    .json(&body)
    .map_err(|e| e.to_string())?
    .send()
    .await
    .map_err(|e| e.to_string())?;
  if !resp.ok() {
    return Err(error_body(resp).await);
  }
  // Learn the clock offset from the response so the *next* op we stamp is in
  // the server's clock domain (best-effort — a body we can't parse just
  // leaves the offset as-is).
  if let Ok(push) = resp.json::<PushResponse>().await {
    crate::clock::observe(push.server_time);
  }
  Ok(())
}

/// Pull all progress rows changed since `since` (epoch millis; `None` = all).
pub async fn pull_progress(
  creds: &Creds,
  since: Option<i64>,
) -> Res<Vec<ProgressDto>> {
  let mut url = api(&creds.server, "/sync/pull");
  if let Some(s) = since {
    url.push_str(&format!("?since={s}"));
  }
  let resp = authed(Request::get(&url), creds)
    .send()
    .await
    .map_err(|e| e.to_string())?;
  if !resp.ok() {
    return Err(error_body(resp).await);
  }
  let pull: PullResponse = resp.json().await.map_err(|e| e.to_string())?;
  // Keep the skew offset fresh from the server's reported time.
  crate::clock::observe(pull.server_time);
  Ok(pull.progress)
}

/// Best-effort human message from a non-2xx response.
async fn error_body(resp: gloo_net::http::Response) -> String {
  let status = resp.status();
  let text = resp.text().await.unwrap_or_default();
  if text.is_empty() {
    format!("server error ({status})")
  } else {
    format!("{status}: {text}")
  }
}
