//! Optional server sync client (progressive enhancement).
//!
//! Built on `reqwest` (rustls). It speaks the bearer-token `/api/v1` JSON API
//! using the shared `hygg-shared` wire DTOs — the same protocol the CLI and PWA
//! use — so a document/position synced here lines up exactly with its twin on
//! another device. Everything is best-effort: failures return `Err(String)` and
//! are surfaced quietly; the reader never depends on the network.

use hygg_shared::sync::headers::{MACHINE_ID_HEADER, USER_HEADER};
use hygg_shared::sync::proto::{
  BookDto, DenialBody, MeResponse, OpPayload, ProgressData, ProgressDto,
  PullResponse, PushRequest, SyncOp,
};

type Res<T> = Result<T, String>;

/// Everything a request needs to authenticate: the server plus the credential
/// triple the API requires — bearer token, account username, and this device's
/// machine id (the token is bound to it). `device_id` tags pushed ops.
#[derive(Clone, Debug)]
pub struct Creds {
  pub server: String,
  pub token: String,
  pub username: String,
  pub machine_id: String,
  pub device_id: String,
}

fn api(server: &str, path: &str) -> String {
  format!("{}/api/v1{}", server.trim_end_matches('/'), path)
}

fn now_ms() -> i64 {
  crate::util::now_ms() as i64
}

/// Attach the auth triple (token + username + machine id) to a request.
fn authed(
  rb: reqwest::RequestBuilder,
  creds: &Creds,
) -> reqwest::RequestBuilder {
  rb.header("Authorization", format!("Bearer {}", creds.token))
    .header(USER_HEADER, &creds.username)
    .header(MACHINE_ID_HEADER, &creds.machine_id)
}

async fn err_body(resp: reqwest::Response) -> String {
  let status = resp.status();
  let text = resp.text().await.unwrap_or_default();
  if text.is_empty() {
    format!("server error ({status})")
  } else {
    format!("{status}: {text}")
  }
}

/// Validate the credentials and report the authenticated principal — the device
/// id and plan. This is how the GUI *connects*: the user pastes a device token
/// created in the server's Devices page (same model as the PWA and the CLI's
/// `:auth`), and this call checks the username + token against `/me`, binding
/// this machine's id to the token on first use and yielding the device id.
pub async fn fetch_me(creds: &Creds) -> Res<MeResponse> {
  let client = reqwest::Client::new();
  let resp = authed(client.get(api(&creds.server, "/me")), creds)
    .send()
    .await
    .map_err(|e| e.to_string())?;
  if !resp.status().is_success() {
    return Err(err_body(resp).await);
  }
  resp.json().await.map_err(|e| e.to_string())
}

/// List documents the connected account can read (metadata only).
pub async fn list_books(creds: &Creds) -> Res<Vec<BookDto>> {
  let client = reqwest::Client::new();
  let resp = authed(client.get(api(&creds.server, "/books")), creds)
    .send()
    .await
    .map_err(|e| e.to_string())?;
  if !resp.status().is_success() {
    return Err(err_body(resp).await);
  }
  resp.json().await.map_err(|e| e.to_string())
}

/// Download a document's raw source bytes by content hash.
pub async fn download_blob(creds: &Creds, book_id: &str) -> Res<Vec<u8>> {
  let client = reqwest::Client::new();
  let resp = authed(
    client.get(api(&creds.server, &format!("/books/{book_id}/blob"))),
    creds,
  )
  .send()
  .await
  .map_err(|e| e.to_string())?;
  if !resp.status().is_success() {
    return Err(err_body(resp).await);
  }
  resp.bytes().await.map(|b| b.to_vec()).map_err(|e| e.to_string())
}

/// Server-side extraction response for a format the GUI can't render itself.
#[derive(serde::Deserialize)]
pub struct ConvertResp {
  pub title: String,
  pub format: String,
  pub text: String,
}

/// Why a server extraction failed. A 403 is kept apart because the server
/// hands back the wording (and any link) to show, which the reader relays.
pub enum ExtractErr {
  Denied(DenialBody),
  Failed(String),
}

/// Fetch the server's canonical extraction of an already-stored document (by
/// content hash) — used to open a format the GUI can't extract locally (DOCX,
/// scanned PDFs). The server reads its retained blob, so the bytes are not
/// re-uploaded.
pub async fn fetch_extraction(
  creds: &Creds,
  book_id: &str,
  col: usize,
) -> Result<ConvertResp, ExtractErr> {
  let client = reqwest::Client::new();
  let resp = authed(
    client.get(api(
      &creds.server,
      &format!("/books/{book_id}/extraction?col={col}"),
    )),
    creds,
  )
  .send()
  .await
  .map_err(|e| ExtractErr::Failed(e.to_string()))?;
  if resp.status().as_u16() == 403 {
    // A server that refuses without a body still yields a usable refusal.
    return Err(ExtractErr::Denied(resp.json().await.unwrap_or_else(|_| {
      DenialBody {
        error: "The server declined this request.".into(),
        action_url: None,
        action_label: None,
      }
    })));
  }
  if !resp.status().is_success() {
    return Err(ExtractErr::Failed(err_body(resp).await));
  }
  resp.json().await.map_err(|e| ExtractErr::Failed(e.to_string()))
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
  let client = reqwest::Client::new();
  let resp =
    authed(client.get(url), creds).send().await.map_err(|e| e.to_string())?;
  if !resp.status().is_success() {
    return Err(err_body(resp).await);
  }
  let pull: PullResponse = resp.json().await.map_err(|e| e.to_string())?;
  Ok(pull.progress)
}

/// Push the latest reading position for a document (last-write-wins by time).
/// `word_offset` is the pagination-independent anchor (non-whitespace character
/// offset of the center line) — the exact resume position for a peer at any
/// width. For PDFs it is page-local, so `page` + `line_in_page` accompany it;
/// both are `None` for reflowable formats.
#[allow(clippy::too_many_arguments)]
pub async fn push_progress(
  creds: &Creds,
  book_id: &str,
  offset: u64,
  total_lines: u64,
  percentage: f64,
  page: Option<u32>,
  line_in_page: Option<u64>,
  word_offset: Option<u64>,
) -> Res<()> {
  let op = SyncOp {
    op_id: uuid::Uuid::new_v4().to_string(),
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
  let device_id =
    (!creds.device_id.is_empty()).then(|| creds.device_id.clone());
  let body = PushRequest { device_id, ops: vec![op] };
  let client = reqwest::Client::new();
  let resp = authed(client.post(api(&creds.server, "/sync/push")), creds)
    .json(&body)
    .send()
    .await
    .map_err(|e| e.to_string())?;
  if resp.status().is_success() { Ok(()) } else { Err(err_body(resp).await) }
}
