//! hygg-tauri — the native shell for the hygg-pwa Leptos UI.
//!
//! ## What runs where
//!
//! The **UI is unchanged**: Tauri renders the same `hygg-pwa` Trunk bundle
//! (`../hygg-pwa/dist`) in the OS webview, so the UX is identical to the
//! browser PWA. The **one architectural change on native** is that the heavy
//! document pipeline moves off wasm into the native IPC commands below: the
//! frontend calls [`extract_document`] instead of running `cli_*` in wasm.
//! Native-speed extraction, no multi-MB wasm cold-compile tax (acute on mobile
//! CPUs).
//!
//! Everything else the PWA does — IndexedDB storage, server sync, Web-Speech
//! TTS, all DOM — stays in the webview untouched.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};

/// Per-line render kind. Serde-compatible (same variant names → same JSON) with
/// `hygg_pwa::model::LineKind`, so it round-trips across IPC into the
/// frontend's `Book` without a translation layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum LineKind {
  /// Justified prose / monospace text.
  Text,
  /// A raw-ANSI truecolor art row (from a PDF image).
  Ansi,
}

/// What the extractor produces for a document — the same pieces the PWA's
/// internal `Extracted` tuple carries. The frontend owns the rest of the `Book`
/// (`title` / `col` / `size_bytes` / `added_at`), exactly as `format::import`
/// does today, and wraps this into a `Book`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Extracted {
  /// `sha256(source_bytes)` (hex) — the stable content id every hygg client
  /// uses.
  pub id: String,
  pub lines: Vec<String>,
  pub kinds: Vec<LineKind>,
  /// `txt` | `epub` | `pdf`.
  pub format: String,
  /// For PDFs: index into `lines` where each 1-based page begins. Empty for
  /// reflowable formats (they sync by percentage).
  pub page_starts: Vec<usize>,
}

/// IPC entry point: base64 file bytes → extracted, justified lines. Mirrors
/// `hygg_pwa::format::import`, but native — the frontend's file input (browser
/// parity) yields bytes, which it base64-encodes for the trip (see
/// `hygg_pwa::tauri_ipc`: a string argument serializes unambiguously where a
/// typed array's `Vec<u8>` mapping is IPC-version-dependent). `col` is the
/// justification width from settings.
#[tauri::command]
fn extract_document(
  filename: String,
  b64: String,
  col: usize,
) -> Result<Extracted, String> {
  let bytes =
    STANDARD.decode(&b64).map_err(|e| format!("Bad file payload: {e}"))?;
  extract_bytes(&filename, &bytes, col)
}

/// The shared extraction core: dispatch on the file extension through the exact
/// hygg pipeline the CLI / PWA / GUI use, so a document renders identically
/// everywhere.
fn extract_bytes(
  filename: &str,
  bytes: &[u8],
  col: usize,
) -> Result<Extracted, String> {
  let id = hygg_shared::sync::content_sha256(bytes);
  let ext = filename
    .rsplit_once('.')
    .map(|(_, e)| e.to_ascii_lowercase())
    .unwrap_or_default();

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

  Ok(Extracted { id, lines, kinds, format, page_starts })
}

type Parts = (Vec<String>, Vec<LineKind>, String, Vec<usize>);

/// Justify plain text into the hygg monospace column (all `Text` rows).
fn justified(text: &str, col: usize, format: &str) -> Parts {
  let lines = cli_justify::justify(text, col);
  let kinds = vec![LineKind::Text; lines.len()];
  (lines, kinds, format.to_string(), Vec::new())
}

/// Extract a PDF to justified lines, preserving ASCII-art rows and the per-page
/// line boundaries used for page-anchored sync.
fn pdf_lines(bytes: &[u8], col: usize) -> Result<Parts, String> {
  use cli_pdf_to_text::PdfLineKind;
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

/// Build and run the Tauri app. Called from `main.rs` on desktop and from the
/// generated mobile entry point on iOS/Android (hence `#[mobile_entry_point]`).
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![extract_document])
    .run(tauri::generate_context!())
    .expect("error while running hygg tauri application");
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn extracts_text_natively_with_stable_id() {
    let bytes = b"Hello world, this is hygg over native IPC. ".repeat(20);
    let ex = extract_bytes("notes.txt", &bytes, 40).expect("extract txt");
    assert_eq!(ex.format, "txt");
    assert!(!ex.lines.is_empty());
    assert_eq!(ex.lines.len(), ex.kinds.len());
    assert!(ex.kinds.iter().all(|k| *k == LineKind::Text));
    // Same content id the CLI / PWA / GUI produce for these bytes — so a
    // document imported natively lines up with its synced twin everywhere.
    assert_eq!(ex.id, hygg_shared::sync::content_sha256(&bytes));
    // Reflowable formats carry no page structure.
    assert!(ex.page_starts.is_empty());
  }

  #[test]
  fn unknown_format_is_a_clean_error() {
    let err = extract_bytes("archive.rar", b"x", 40).unwrap_err();
    assert!(err.contains("rar"));
  }

  /// Exercise the real native PDF pipeline (`cli_pdf_to_text`) end-to-end when
  /// a repo test fixture is present. Skips gracefully otherwise so the test
  /// stays CI-robust regardless of the fixture set.
  #[test]
  fn extracts_pdf_pages_natively() {
    let path =
      concat!(env!("CARGO_MANIFEST_DIR"), "/../test-data/pdf/progit-1-50.pdf");
    let Ok(bytes) = std::fs::read(path) else {
      eprintln!("fixture {path} absent — skipping");
      return;
    };
    let ex = extract_bytes("progit-1-50.pdf", &bytes, 72).expect("extract pdf");
    assert_eq!(ex.format, "pdf");
    assert!(ex.lines.iter().filter(|l| !l.trim().is_empty()).count() >= 3);
    // A real PDF carries page provenance for page-anchored sync.
    assert!(!ex.page_starts.is_empty());
  }

  /// Native EPUB extraction (`cli_epub_to_text`), fixture-gated like the PDF
  /// case so it stays CI-robust.
  #[test]
  fn extracts_epub_natively() {
    let path = concat!(
      env!("CARGO_MANIFEST_DIR"),
      "/../test-data/epub/test-standard.epub"
    );
    let Ok(bytes) = std::fs::read(path) else {
      eprintln!("fixture {path} absent — skipping");
      return;
    };
    let ex =
      extract_bytes("test-standard.epub", &bytes, 72).expect("extract epub");
    assert_eq!(ex.format, "epub");
    assert!(!ex.lines.is_empty());
    // Reflowable — no page structure.
    assert!(ex.page_starts.is_empty());
  }

  /// The `extract_document` command decodes base64 and produces the same result
  /// as the raw core — covers the IPC payload path end-to-end (minus the
  /// webview transport, which is a plain string round-trip).
  #[test]
  fn extract_document_decodes_base64() {
    let bytes = b"a hygg document, base64 round-tripped. ".repeat(10);
    let b64 = STANDARD.encode(&bytes);
    let ex =
      extract_document("notes.txt".into(), b64, 40).expect("decode + extract");
    assert_eq!(ex.id, hygg_shared::sync::content_sha256(&bytes));
    assert_eq!(ex.format, "txt");
    assert!(!ex.lines.is_empty());
  }
}
