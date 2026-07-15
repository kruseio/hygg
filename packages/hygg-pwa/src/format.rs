//! Client-side document import: raw bytes → extracted, justified [`Book`].
//!
//! Reuses the hygg pipeline compiled to wasm — `cli_justify` for the signature
//! justified monospace look, `cli_epub_to_text` / `cli_pdf_to_text` for format
//! extraction. TXT/EPUB/text-PDF work fully offline here; scanned PDFs and
//! pandoc formats (DOCX/…) are left to the server (progressive enhancement).

use cli_pdf_to_text::PdfLineKind;

use crate::model::{Book, LineKind};

/// The pieces an extractor produces for a document: the rendered lines, their
/// per-line kinds, the format tag, and the PDF page boundaries (empty for
/// reflowable formats).
type Extracted = (Vec<String>, Vec<LineKind>, String, Vec<usize>);

/// Build a [`Book`] from an imported file's bytes. `col` is the justification
/// width (from settings). Returns a user-facing error string on failure.
///
/// Dispatches on the runtime: inside the Tauri shell the extraction runs as
/// native Rust over IPC (`crate::tauri_ipc`); in a plain browser it runs the
/// same pipeline compiled to wasm. Either way the resulting [`Book`] is
/// identical — same content id, lines, and page anchors — so storage, sync, and
/// the reader are oblivious to which path produced it.
pub async fn import(
  filename: &str,
  bytes: &[u8],
  col: usize,
) -> Result<Book, String> {
  let (id, parts) = if crate::tauri_ipc::in_tauri() {
    let ex = crate::tauri_ipc::extract_document(filename, bytes, col).await?;
    (ex.id, (ex.lines, ex.kinds, ex.format, ex.page_starts))
  } else {
    (
      hygg_shared::sync::content_sha256(bytes),
      extract_local(filename, bytes, col)?,
    )
  };
  Ok(assemble(filename, bytes, col, id, parts))
}

/// Run the wasm-side extraction pipeline (browser path). The Tauri path calls
/// the native equivalent over IPC instead; both return the same [`Extracted`].
fn extract_local(
  filename: &str,
  bytes: &[u8],
  col: usize,
) -> Result<Extracted, String> {
  match extension(filename).as_str() {
    "pdf" => pdf_lines(bytes, col),
    "epub" => {
      let text = cli_epub_to_text::epub_bytes_to_text(bytes)
        .map_err(|e| format!("Couldn't read EPUB: {e}"))?;
      Ok(justified(&text, col, "epub"))
    }
    "txt" | "text" | "md" | "markdown" => {
      let text = String::from_utf8_lossy(bytes).into_owned();
      Ok(justified(&text, col, "txt"))
    }
    other => Err(format!(
      "Can't open .{other} offline yet — connect a server to convert it."
    )),
  }
}

/// Wrap extracted [`Extracted`] parts into a [`Book`], filling the fields the
/// frontend owns regardless of extraction path (title from the filename, the
/// requested column, byte size, and the import timestamp).
fn assemble(
  filename: &str,
  bytes: &[u8],
  col: usize,
  id: String,
  (lines, kinds, format, page_starts): Extracted,
) -> Book {
  Book {
    id,
    title: title_from_filename(filename),
    format,
    col,
    lines,
    kinds,
    size_bytes: bytes.len(),
    added_at: js_sys::Date::now(),
    page_starts,
  }
}

/// Build a [`Book`] from server-converted text (already justified by the
/// server). ASCII-art rows are detected by an embedded ANSI escape per line.
pub fn book_from_server_text(
  title: &str,
  format: &str,
  text: &str,
  bytes: &[u8],
) -> Book {
  let mut lines = Vec::new();
  let mut kinds = Vec::new();
  for line in text.split('\n') {
    kinds.push(if line.contains('\u{1b}') {
      LineKind::Ansi
    } else {
      LineKind::Text
    });
    lines.push(line.to_string());
  }
  Book {
    id: hygg_shared::sync::content_sha256(bytes),
    title: title.to_string(),
    format: format.to_string(),
    col: 0,
    lines,
    kinds,
    size_bytes: bytes.len(),
    added_at: js_sys::Date::now(),
    // Server-converted text (OCR / pandoc fallback) has no page structure, so
    // it can't page-anchor and falls back to percentage sync.
    page_starts: Vec::new(),
  }
}

/// Justify plain text into the hygg monospace column (all `Text` rows). No page
/// structure — reflowable formats sync by percentage.
fn justified(text: &str, col: usize, format: &str) -> Extracted {
  let lines = cli_justify::justify(text, col);
  let kinds = vec![LineKind::Text; lines.len()];
  (lines, kinds, format.to_string(), Vec::new())
}

/// Extract a PDF to justified lines while preserving which rows are ASCII-art,
/// plus the per-page line boundaries used for page-anchored sync.
fn pdf_lines(bytes: &[u8], col: usize) -> Result<Extracted, String> {
  let (rows, page_starts) =
    cli_pdf_to_text::pdf_bytes_to_lines_paged(bytes.to_vec(), col)
      .map_err(|e| format!("Couldn't read PDF: {e}"))?;
  let mut lines = Vec::with_capacity(rows.len());
  let mut kinds = Vec::with_capacity(rows.len());
  for (line, kind) in rows {
    lines.push(line);
    kinds.push(match kind {
      PdfLineKind::Text => LineKind::Text,
      PdfLineKind::AnsiArt => LineKind::Ansi,
    });
  }
  Ok((lines, kinds, "pdf".to_string(), page_starts))
}

/// Lowercased extension without the dot (`""` if none).
fn extension(filename: &str) -> String {
  filename
    .rsplit_once('.')
    .map(|(_, ext)| ext.to_ascii_lowercase())
    .unwrap_or_default()
}

/// Human title: strip any directory prefix and the extension.
fn title_from_filename(filename: &str) -> String {
  let base = filename.rsplit(['/', '\\']).next().unwrap_or(filename);
  base.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(base).to_string()
}
