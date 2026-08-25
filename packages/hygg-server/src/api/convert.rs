//! `POST /api/v1/convert` — server-side document extraction for formats the
//! browser PWA can't handle offline: scanned-PDF OCR (bundled tract engine) and
//! pandoc formats (DOCX/ODT/RTF/…). Gated like the rest of the sync API, so an
//! override can answer 403 here and have the client relay its wording. The raw
//! upload is the request body; `?filename=&col=` carry the name + width.
//!
//! Extraction is CPU-heavy (OCR especially), so it runs on a blocking thread.

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Query, State};
use hygg_shared::sync::content_sha256;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::middleware::entitlement::SyncPrincipal;
use crate::repo;
use crate::state::AppState;

/// The document-extraction pipeline version. Every cached extraction is stamped
/// with this; bumping it (after a change to any extractor/justifier that alters
/// output) makes every older cache row a miss, so the next `/convert` re-runs
/// the pipeline and overwrites it — the re-render path, since the original
/// bytes are retained in `book_blobs`.
pub const EXTRACTOR_VERSION: i64 = 1;

/// Justification width the background upload pre-warm renders at. Matches the
/// clients' default `import_col`, so the common case is a cache hit; other
/// widths are computed and cached on demand by `/convert`.
pub const PREWARM_COL: usize = 64;

#[derive(Deserialize)]
pub struct ConvertQuery {
  pub filename: String,
  #[serde(default = "default_col")]
  pub col: usize,
}

fn default_col() -> usize {
  64
}

#[derive(Serialize)]
pub struct ConvertResponse {
  pub title: String,
  pub format: String,
  /// Justified text, newline-joined; ASCII-art rows carry embedded ANSI.
  pub text: String,
}

pub async fn convert(
  SyncPrincipal(principal): SyncPrincipal,
  State(state): State<AppState>,
  Query(q): Query<ConvertQuery>,
  body: Bytes,
) -> AppResult<Json<ConvertResponse>> {
  if body.is_empty() {
    return Err(AppError::BadRequest("empty upload".to_string()));
  }
  // An encrypted envelope carries no extractable text — the server has no key.
  // Refuse clearly rather than run OCR/pandoc over ciphertext and cache noise;
  // encrypted documents are extracted on the client.
  if hygg_shared::crypto::is_envelope(&body) {
    return Err(AppError::BadRequest(
      "cannot extract an encrypted document server-side; extract it on a \
       client that holds the key"
        .to_string(),
    ));
  }
  let use_cache = state.config.extraction_cache;
  let content_hash = content_sha256(&body);
  let col = q.col as i64;

  // Return a cached extraction (same document, pipeline version, and width)
  // without re-running OCR/pandoc.
  if use_cache
    && let Some(hit) = repo::extractions::get(
      &state.db.conn,
      &principal.tenant_id,
      &content_hash,
      EXTRACTOR_VERSION,
      col,
    )
    .await?
  {
    return Ok(Json(ConvertResponse {
      title: hit.title,
      format: hit.format,
      text: hit.text,
    }));
  }

  // Miss: run the CPU-heavy extraction on a blocking thread, then cache it.
  // Take a conversion permit first. Each conversion pins a blocking thread for
  // seconds (OCR, or a pandoc child), so without a bound a burst of uploads
  // starves the pool and the CPU. `try_acquire` rather than `acquire`: a queued
  // request would sit here holding its entire decoded body, so refuse with 429
  // and let the client back off instead.
  let _permit = state
    .convert_slots
    .clone()
    .try_acquire_owned()
    .map_err(|_| AppError::TooManyRequests)?;
  let bytes = body.to_vec();
  let filename = q.filename;
  let width = q.col;
  let resp =
    tokio::task::spawn_blocking(move || convert_bytes(&filename, bytes, width))
      .await
      .map_err(|_| AppError::Internal)??;

  if use_cache {
    // Best-effort: a cache-write failure must not fail the conversion.
    let _ = repo::extractions::put(
      &state.db.conn,
      &principal.tenant_id,
      &content_hash,
      EXTRACTOR_VERSION,
      col,
      &repo::extractions::CachedExtraction {
        title: resp.title.clone(),
        format: resp.format.clone(),
        text: resp.text.clone(),
      },
    )
    .await;
  }
  Ok(Json(resp))
}

/// Best-effort background pre-warm of the extraction cache for a freshly
/// uploaded document (called from the blob-upload path): run the
/// OCR/pandoc/justify pipeline once at the default width and store the
/// canonical text, so a later `/convert` — or a thin client — reuses it instead
/// of re-extracting. Never blocks or fails the upload; skips work already
/// cached, and no-ops when the document has no book row or no extractable
/// format. Only callers behind the `extraction_cache` flag should invoke this.
pub fn spawn_prewarm_extraction(
  state: &AppState,
  tenant_id: &str,
  content_hash: &str,
  bytes: Vec<u8>,
) {
  let pool = state.db.conn.clone();
  let slots = state.convert_slots.clone();
  let tenant_id = tenant_id.to_string();
  let content_hash = content_hash.to_string();
  let col = PREWARM_COL as i64;
  tokio::spawn(async move {
    // Already extracted at this version+width (e.g. the client called /convert
    // before uploading)? Nothing to do.
    match repo::extractions::get(
      &pool,
      &tenant_id,
      &content_hash,
      EXTRACTOR_VERSION,
      col,
    )
    .await
    {
      Ok(Some(_)) | Err(_) => return,
      Ok(None) => {}
    }
    let Ok(Some((format, file_name))) =
      repo::books::extract_hint(&pool, &tenant_id, &content_hash).await
    else {
      return;
    };
    let filename = file_name.unwrap_or_else(|| format!("document.{format}"));
    // Share the conversion budget with `/convert`. This is background work, so
    // it waits for a permit rather than refusing; if the semaphore is gone
    // (shutdown) there is nothing to warm.
    let Ok(_permit) = slots.acquire_owned().await else {
      return;
    };
    let extracted = tokio::task::spawn_blocking(move || {
      convert_bytes(&filename, bytes, PREWARM_COL)
    })
    .await;
    if let Ok(Ok(resp)) = extracted {
      let _ = repo::extractions::put(
        &pool,
        &tenant_id,
        &content_hash,
        EXTRACTOR_VERSION,
        col,
        &repo::extractions::CachedExtraction {
          title: resp.title,
          format: resp.format,
          text: resp.text,
        },
      )
      .await;
    }
  });
}

/// Run the format-appropriate extractor over an in-memory document and justify
/// the result to `col`. CPU-heavy (OCR/pandoc); call on a blocking thread.
pub fn convert_bytes(
  filename: &str,
  bytes: Vec<u8>,
  col: usize,
) -> AppResult<ConvertResponse> {
  let ext = extension(filename);
  let text = match ext.as_str() {
    "pdf" => {
      cli_pdf_to_text::pdf_bytes_to_ansi_text_with_bundled_ocr(bytes, col)
        .map_err(|e| AppError::BadRequest(format!("PDF OCR failed: {e}")))?
    }
    "epub" => {
      let t = cli_epub_to_text::epub_bytes_to_text(&bytes)
        .map_err(|e| AppError::BadRequest(format!("EPUB failed: {e}")))?;
      cli_justify::justify(&t, col).join("\n")
    }
    "txt" | "text" | "md" | "markdown" => {
      cli_justify::justify(&String::from_utf8_lossy(&bytes), col).join("\n")
    }
    other => super::pandoc::pandoc_convert(&bytes, other, col)?,
  };
  Ok(ConvertResponse { title: title_from(filename), format: ext, text })
}

fn extension(filename: &str) -> String {
  filename
    .rsplit_once('.')
    .map(|(_, e)| e.to_ascii_lowercase())
    .unwrap_or_default()
}

fn title_from(filename: &str) -> String {
  let base = filename.rsplit(['/', '\\']).next().unwrap_or(filename);
  base.rsplit_once('.').map(|(s, _)| s).unwrap_or(base).to_string()
}
