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

use crate::auth::Principal;
use crate::error::{AppError, AppResult};
use crate::repo;
use crate::state::AppState;
use crate::util::{new_id, now_millis};

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
  let tenant = &principal.tenant_id;
  let user = &principal.user_id;
  let mut summary = ImportSummary::default();
  for book in bundle.books {
    import_book(&state, tenant, user, book, &mut summary).await?;
  }
  Ok(Json(summary))
}

/// Restore one document and everything attached to it into the caller's
/// library.
async fn import_book(
  state: &AppState,
  tenant: &str,
  user: &str,
  book: ExportBook,
  summary: &mut ImportSummary,
) -> AppResult<()> {
  let pool = &state.db.conn;
  let hash = book.content_hash.as_str();
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

/// Imported annotations are not tied to a device (the export dropped device
/// identity) and get a fresh idempotency id; `updated_at` is preserved so
/// last-write-wins keeps the exporting server's ordering.
fn progress_input(dto: ProgressDto) -> repo::progress::ProgressInput {
  repo::progress::ProgressInput {
    book_id: dto.book_id,
    device_id: None,
    offset_line: dto.offset_line,
    total_lines: dto.total_lines,
    percentage: dto.percentage,
    viewport_offset: dto.viewport_offset,
    cursor_y: dto.cursor_y,
    page: dto.page,
    line_in_page: dto.line_in_page,
    word_offset: dto.word_offset,
    op_id: new_id(),
    updated_at: dto.updated_at,
  }
}

fn bookmark_input(dto: BookmarkDto) -> repo::bookmarks::BookmarkInput {
  repo::bookmarks::BookmarkInput {
    book_id: dto.book_id,
    device_id: None,
    mark: dto.mark,
    line: dto.line,
    col: dto.col,
    op_id: new_id(),
    deleted: dto.deleted,
    updated_at: dto.updated_at,
  }
}

fn highlight_input(dto: HighlightDto) -> repo::highlights::HighlightInput {
  repo::highlights::HighlightInput {
    book_id: dto.book_id,
    device_id: None,
    start_offset: dto.start_offset,
    end_offset: dto.end_offset,
    op_id: new_id(),
    deleted: dto.deleted,
    created_at: dto.updated_at,
    updated_at: dto.updated_at,
  }
}

fn note_input(dto: NoteDto) -> repo::notes::NoteInput {
  repo::notes::NoteInput {
    note_uid: dto.id,
    book_id: dto.book_id,
    device_id: None,
    anchor_line: dto.anchor_line,
    body: dto.body,
    op_id: new_id(),
    deleted: dto.deleted,
    created_at: dto.created_at,
    updated_at: dto.updated_at,
  }
}
