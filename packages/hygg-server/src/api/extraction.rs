//! `GET /api/v1/books/{content_hash}/extraction` — the server's canonical text
//! extraction of an already-stored document, for clients that can't extract the
//! format themselves (the browser PWA opening a DOCX or scanned PDF that was
//! synced from another device). It reads the *retained source blob*, runs the
//! same pipeline as `/convert`, and reuses the extraction cache — so OCR/pandoc
//! runs at most once per `(document, width)` and a thin client never has to
//! re-upload bytes the server already holds.
//!
//! Entitlement-gated like `/convert` (via [`SyncPrincipal`]); per-document read
//! access is enforced exactly as for a blob download.

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::Deserialize;

use crate::api::convert::{ConvertResponse, EXTRACTOR_VERSION, convert_bytes};
use crate::error::{AppError, AppResult};
use crate::middleware::entitlement::SyncPrincipal;
use crate::repo;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct ExtractionQuery {
  /// Justification width; defaults to the clients' standard column.
  #[serde(default = "default_col")]
  pub col: usize,
}

fn default_col() -> usize {
  64
}

pub async fn get_extraction(
  SyncPrincipal(principal): SyncPrincipal,
  State(state): State<AppState>,
  Path(content_hash): Path<String>,
  Query(q): Query<ExtractionQuery>,
) -> AppResult<Json<ConvertResponse>> {
  if !principal.can_read_book(&content_hash) {
    return Err(AppError::Forbidden);
  }
  let access = repo::access::library_for_hash(
    &state.db.conn,
    state.entitlements.as_ref(),
    &principal.tenant_id,
    &principal.user_id,
    principal.role.is_admin(),
    principal.personal_sync,
    Some(&principal.device_id),
    &content_hash,
  )
  .await?;
  if !access.can_read() {
    return Err(AppError::NotFound);
  }

  let use_cache = state.config.extraction_cache;
  let col = q.col as i64;
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

  // Miss: extract from the retained source blob (which the caller already has
  // read access to). No blob (metadata-only document) is a 404.
  let book_id = repo::books::find_id_by_hash(
    &state.db.conn,
    &principal.tenant_id,
    &content_hash,
  )
  .await?
  .ok_or(AppError::NotFound)?;
  let bytes = repo::blobs::get(&state.db.conn, &principal.tenant_id, &book_id)
    .await?
    .ok_or(AppError::NotFound)?;
  let (format, file_name) = repo::books::extract_hint(
    &state.db.conn,
    &principal.tenant_id,
    &content_hash,
  )
  .await?
  .ok_or(AppError::NotFound)?;
  let filename = file_name.unwrap_or_else(|| format!("document.{format}"));
  let width = q.col;
  let resp =
    tokio::task::spawn_blocking(move || convert_bytes(&filename, bytes, width))
      .await
      .map_err(|_| AppError::Internal)??;

  if use_cache {
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
