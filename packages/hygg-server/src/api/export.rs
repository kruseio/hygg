//! Full per-user data export/import — the migration path between any two
//! deployments, in either direction. `export` serialises the caller's personal
//! library (document metadata + bytes + tags + every annotation) into a
//! portable [`ExportBundle`]; `import` merges a bundle back into the caller's
//! account.
//!
//! Authenticated with a bare [`Principal`] rather than the `SyncPrincipal`
//! gate: exporting and re-importing your own data is a portability right, not a
//! synced feature, so it must work even for an account that may not sync.
//!
//! Organization documents are deliberately out of scope — they belong to the
//! organization, not the user — so a bundle is a clean *personal* library.

use std::collections::HashMap;

use axum::Json;
use axum::extract::State;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use hygg_shared::export::{
  EXPORT_FORMAT_VERSION, ExportBook, ExportBundle, ExportProfile, ImportSummary,
};
use hygg_shared::sync::content_sha256;
use hygg_shared::sync::proto::{
  BookmarkDto, HighlightDto, NoteDto, ProgressDto,
};

use super::export_inputs::{
  bookmark_input, highlight_input, note_input, progress_input,
};
use crate::auth::Principal;
use crate::error::{AppError, AppResult};
use crate::repo;
use crate::state::AppState;
use crate::util::now_millis;

/// `GET /api/v1/export` — the caller's complete personal library as a bundle.
pub async fn export(
  principal: Principal,
  State(state): State<AppState>,
) -> AppResult<Json<ExportBundle>> {
  let pool = &state.db.conn;
  let tenant = &principal.tenant_id;
  let user = &principal.user_id;

  let row = repo::users::find_by_id(pool, tenant, user)
    .await?
    .ok_or(AppError::Unauthorized)?;
  let profile = ExportProfile { email: row.email, name: row.display_name };

  // Index every annotation by book id (= content hash) once, so attaching them
  // to each owned book below is a single map lookup rather than a rescan.
  let mut progress_by: HashMap<String, ProgressDto> =
    repo::progress::list_for_user(pool, tenant, user)
      .await?
      .into_iter()
      .map(|r| (r.book_id.clone(), r.into()))
      .collect();
  let mut bookmarks_by = group_by_book::<_, BookmarkDto>(
    repo::bookmarks::list_since(pool, tenant, user, 0).await?,
  );
  let mut highlights_by = group_by_book::<_, HighlightDto>(
    repo::highlights::list_since(pool, tenant, user, 0).await?,
  );
  let mut notes_by = group_by_book::<_, NoteDto>(
    repo::notes::list_since(pool, tenant, user, 0).await?,
  );
  let mut tags_by: HashMap<String, Vec<String>> = HashMap::new();
  for tag in repo::tags::visible_book_tags(pool, tenant, user, &[]).await? {
    tags_by.entry(tag.content_hash).or_default().push(tag.name);
  }

  let mut books = Vec::new();
  for book in repo::books::list_for_user(pool, tenant, user).await? {
    // Personal library only.
    if book.organization_id.is_some() || book.owner_user_id != *user {
      continue;
    }
    let hash = book.content_hash;
    let blob_base64 =
      match repo::books::find_id_by_hash(pool, tenant, &hash).await? {
        Some(book_id) => repo::blobs::get(pool, tenant, &book_id)
          .await?
          .map(|bytes| STANDARD.encode(bytes)),
        None => None,
      };
    books.push(ExportBook {
      title: book.title,
      author: book.author,
      format: book.format,
      size_bytes: book.size_bytes,
      file_name: book.file_name,
      tags: tags_by.remove(&hash).unwrap_or_default(),
      blob_base64,
      progress: progress_by.remove(&hash),
      bookmarks: bookmarks_by.remove(&hash).unwrap_or_default(),
      highlights: highlights_by.remove(&hash).unwrap_or_default(),
      notes: notes_by.remove(&hash).unwrap_or_default(),
      content_hash: hash,
    });
  }

  Ok(Json(ExportBundle {
    format_version: EXPORT_FORMAT_VERSION,
    exported_at: now_millis(),
    profile,
    books,
  }))
}

/// Group annotation rows by their `book_id`, converting each row to its DTO.
fn group_by_book<R, D>(rows: Vec<R>) -> HashMap<String, Vec<D>>
where
  D: From<R> + HasBookId,
{
  let mut out: HashMap<String, Vec<D>> = HashMap::new();
  for row in rows {
    let dto = D::from(row);
    out.entry(dto.book_id().to_string()).or_default().push(dto);
  }
  out
}

/// The `book_id` accessor the grouping above needs from each annotation DTO.
trait HasBookId {
  fn book_id(&self) -> &str;
}
impl HasBookId for BookmarkDto {
  fn book_id(&self) -> &str {
    &self.book_id
  }
}
impl HasBookId for HighlightDto {
  fn book_id(&self) -> &str {
    &self.book_id
  }
}
impl HasBookId for NoteDto {
  fn book_id(&self) -> &str {
    &self.book_id
  }
}

/// `POST /api/v1/import` — merge a bundle into the caller's account.
/// Idempotent: re-importing the same bundle is a no-op (upserts are
/// last-write-wins by `updated_at`, and metadata/tags conflict-resolve to the
/// same row).
pub async fn import(
  principal: Principal,
  State(state): State<AppState>,
  Json(bundle): Json<ExportBundle>,
) -> AppResult<Json<ImportSummary>> {
  if bundle.format_version != EXPORT_FORMAT_VERSION {
    return Err(AppError::BadRequest(format!(
      "unsupported export format version {}",
      bundle.format_version
    )));
  }
  let mut summary = ImportSummary::default();
  for book in bundle.books {
    import_book(&state, &principal, book, &mut summary).await?;
  }
  Ok(Json(summary))
}

/// Restore one document and everything attached to it into the caller's
/// library.
async fn import_book(
  state: &AppState,
  principal: &Principal,
  book: ExportBook,
  summary: &mut ImportSummary,
) -> AppResult<()> {
  let pool = &state.db.conn;
  let (tenant, user) = (&principal.tenant_id, &principal.user_id);
  let hash = book.content_hash.as_str();

  // A book is unique per *tenant* by its content hash (`uq_books_tenant_hash`),
  // not per owner, so a bundle entry can name a document that already exists
  // and belongs to someone else. The upsert below preserves that row's owner
  // while overwriting its metadata, and the blob put further down replaces
  // its stored bytes — so without a check here, any authenticated user who
  // learns a hash (a shared document, an org book, a guessed common file)
  // could overwrite another user's document through import. The regular
  // upload path guards the same write with
  // `library_for_hash(...).can_write()`; require it here too.
  //
  // Only when the book already exists: an unknown hash is a fresh personal
  // import (the migration path this endpoint exists for), and
  // `library_for_hash` returns `None` for a hash the tenant does not have —
  // gating on that would reject every new import. The content hash is not
  // verified against the blob bytes on purpose: a book id is often the
  // SHA-256 of the document's extracted *text* (`book_id_from_text`), not of
  // the uploaded file bytes, so the two do not match in general — the regular
  // blob upload does not check it either.
  if repo::books::access_meta(pool, tenant, hash).await?.is_some() {
    let access = repo::access::library_for_hash(
      pool,
      state.entitlements.as_ref(),
      tenant,
      user,
      principal.role.is_admin(),
      principal.personal_sync,
      Some(&principal.device_id),
      hash,
    )
    .await?;
    if !access.can_write() {
      return Err(AppError::Forbidden);
    }
  }

  repo::books::upsert(
    pool,
    tenant,
    user,
    &repo::books::BookInput {
      content_hash: hash,
      title: &book.title,
      author: &book.author,
      format: &book.format,
      size_bytes: book.size_bytes,
    },
  )
  .await?;
  summary.books += 1;

  if let Some(encoded) = &book.blob_base64 {
    let bytes = STANDARD
      .decode(encoded)
      .map_err(|_| AppError::BadRequest("invalid base64 blob".into()))?;
    if let Some(book_id) =
      repo::books::find_id_by_hash(pool, tenant, hash).await?
    {
      let sha256 = content_sha256(&bytes);
      repo::blobs::put(pool, tenant, &book_id, &bytes, &sha256).await?;
      summary.blobs += 1;
    }
  }

  for name in &book.tags {
    let tag_id = repo::tags::ensure(pool, tenant, "user", user, name).await?;
    repo::tags::attach(pool, tenant, &tag_id, hash).await?;
    summary.tags += 1;
  }

  if let Some(progress) = book.progress {
    repo::progress::upsert(pool, tenant, user, &progress_input(progress))
      .await?;
    summary.progress += 1;
  }
  for bookmark in book.bookmarks {
    repo::bookmarks::upsert(pool, tenant, user, &bookmark_input(bookmark))
      .await?;
    summary.bookmarks += 1;
  }
  for highlight in book.highlights {
    repo::highlights::upsert(pool, tenant, user, &highlight_input(highlight))
      .await?;
    summary.highlights += 1;
  }
  for note in book.notes {
    repo::notes::upsert(pool, tenant, user, &note_input(note)).await?;
    summary.notes += 1;
  }
  Ok(())
}
