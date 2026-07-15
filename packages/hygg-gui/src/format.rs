//! Document import: raw bytes → extracted, justified [`Book`].
//!
//! Reuses the hygg pipeline (`cli_justify` for the signature justified
//! monospace look, `cli_epub_to_text` / `cli_pdf_to_text` for format
//! extraction), so a document rendered here is byte-for-byte the same column
//! the CLI and PWA show. TXT/MD/EPUB/text-PDF all work fully offline; scanned
//! PDFs and pandoc formats (DOCX/…) are left to an optional server (progressive
//! enhancement).

use cli_pdf_to_text::PdfLineKind;

use crate::model::{Book, LineKind};
use crate::util::now_ms;

/// The pieces an extractor produces for a document: the rendered lines, their
/// per-line kinds, the format tag, and the PDF page boundaries (empty for
/// reflowable formats).
type Extracted = (Vec<String>, Vec<LineKind>, String, Vec<usize>);

/// Build a [`Book`] from an imported file's bytes. `col` is the justification
/// width (from settings). Returns a user-facing error string on failure.
pub fn import(
  filename: &str,
  bytes: &[u8],
  col: usize,
) -> Result<Book, String> {
  let id = hygg_shared::sync::content_sha256(bytes);
  let title = title_from_filename(filename);
  let ext = extension(filename);

  let (lines, kinds, format, page_starts) = match ext.as_str() {
    "pdf" => pdf_lines(bytes, col)?,
    "epub" => {
      let text = cli_epub_to_text::epub_bytes_to_text(bytes)
        .map_err(|e| format!("Couldn't read EPUB: {e}"))?;
      justified(&text, col, "epub")
    }
    "txt" | "text" | "md" | "markdown" => {
      let text = String::from_utf8_lossy(bytes).into_owned();
      justified(&text, col, "txt")
    }
    other => {
      return Err(format!(
        "Can't open .{other} offline yet — connect a server to convert it."
      ));
    }
  };

  Ok(Book {
    id,
    title,
    format,
    col,
    lines,
    kinds,
    size_bytes: bytes.len(),
    added_at: now_ms(),
    page_starts,
  })
}

/// Build a [`Book`] from server-converted text (already justified by the
/// server to `col`, for a format the GUI can't extract itself — DOCX, scanned
/// PDFs). ASCII-art rows are detected by an embedded ANSI escape per line.
/// `col` is the width the server wrapped to, so the reader centers the block
/// the same as a local document. Server text has no page structure (no
/// `page_starts`), so it still syncs by percentage.
pub fn book_from_server_text(
  title: &str,
  format: &str,
  text: &str,
  bytes: &[u8],
  col: usize,
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
    col,
    lines,
    kinds,
    size_bytes: bytes.len(),
    added_at: now_ms(),
    page_starts: Vec::new(),
  }
}

/// At least a few non-empty lines — a quick "did extraction work" check (a
/// scanned PDF extracts to near-nothing and should fall through to the server).
pub fn has_text(book: &Book) -> bool {
  book.lines.iter().filter(|l| !l.trim().is_empty()).count() >= 3
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

/// Extensions the file picker offers and the app claims as a document handler.
/// Kept in one place so the picker filter, the desktop MIME registration and
/// the docs stay in sync.
pub const SUPPORTED_EXTENSIONS: &[&str] =
  &["pdf", "epub", "txt", "text", "md", "markdown"];

#[cfg(test)]
mod tests {
  use super::*;
  use crate::model::LineKind;

  #[test]
  fn imports_text_into_a_justified_book() {
    let bytes = b"Hello world, this is hygg. ".repeat(20);
    let book = import("notes.txt", &bytes, 40).expect("import txt");
    assert_eq!(book.format, "txt");
    assert_eq!(book.col, 40);
    assert!(!book.lines.is_empty());
    assert_eq!(book.lines.len(), book.kinds.len());
    assert!(book.kinds.iter().all(|k| *k == LineKind::Text));
    // Stable content identity — same bytes, same id as everywhere else in hygg.
    assert_eq!(book.id, hygg_shared::sync::content_sha256(&bytes));
    // Reflowable formats carry no page structure.
    assert!(!book.has_pages());
  }

  #[test]
  fn title_strips_directory_and_extension() {
    let book = import("/a/b/War and Peace.md", b"x", 40).unwrap();
    assert_eq!(book.title, "War and Peace");
  }

  #[test]
  fn unknown_format_is_a_clean_error() {
    let err = import("archive.rar", b"x", 40).unwrap_err();
    assert!(err.contains("rar"));
  }
}
