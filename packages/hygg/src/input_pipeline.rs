mod content;
mod pandoc;

use crate::{args::Args, binary_lookup::which};
pub(crate) use content::Content;
use pandoc::pandoc_to_text;
use std::io::{self, IsTerminal, Read};

pub(crate) struct PreparedInput {
  pub(crate) lines: Vec<String>,
  pub(crate) temp_file: Option<String>,
  /// The document's identity text: the reader hashes it into the local
  /// `document_hash` (progress, highlights, bookmarks, notes) and the sync
  /// `book_id`. See [`Content::identity`]; it is not necessarily the text
  /// that was justified into `lines`.
  pub(crate) raw_content: Option<String>,
  /// Path the document was read from, when it came from a file (not stdin).
  /// Threaded into the reader so the open is recorded in the `:home` library.
  pub(crate) source_path: Option<String>,
}

/// Read piped stdin. Returns its identity text (see [`Content::identity`]):
/// the input exactly as before for UTF-8, otherwise the decoded text — so
/// UTF-16 or legacy-encoded input is no longer mistaken for "no input".
pub(crate) fn read_stdin_content() -> Option<String> {
  if io::stdin().is_terminal() {
    return None;
  }

  let mut buffer = Vec::new();
  io::stdin().read_to_end(&mut buffer).ok()?;
  stdin_identity(buffer)
}

fn stdin_identity(bytes: Vec<u8>) -> Option<String> {
  if bytes.is_empty() {
    return None;
  }
  let len = bytes.len();
  let content = Content::from_bytes(bytes, "standard input");
  debug_log_encoding("stdin", len, &content);
  Some(content.identity)
}

/// HYGG_DEBUG only: the encoding detected for a plain-text source, logged here
/// where the original bytes were decoded rather than decoding them again.
fn debug_log_encoding(source: &str, len: usize, content: &Content) {
  if let Some(encoding) = &content.encoding {
    cli_text_reader::debug_log(
      "startup",
      &format!("input: {source}, {len} bytes, encoding {encoding}"),
    );
  }
}

pub(crate) fn prepare_input(
  args: &Args,
  stdin_content: Option<String>,
  ocr_enabled: bool,
) -> Result<PreparedInput, Box<dyn std::error::Error>> {
  if let Some(identity) = stdin_content {
    // Decoding the identity again yields the display text: for UTF-8 input it
    // strips the BOM and normalises line endings; for input that was decoded
    // from another encoding it is already clean, and decoding is a no-op.
    let display = Content::from_bytes(identity.clone().into_bytes(), "stdin");
    return Ok(PreparedInput {
      lines: cli_justify::justify(&display.display, args.col),
      temp_file: None,
      raw_content: Some(identity),
      source_path: None,
    });
  }

  let Some(file) = resolve_input_file(args.file.clone()) else {
    return Ok(PreparedInput {
      lines: vec![],
      temp_file: None,
      raw_content: None,
      source_path: None,
    });
  };

  process_file_input(args, &file, ocr_enabled)
}

pub(crate) fn cleanup_temp_file(
  temp_file: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
  if let Some(path) = temp_file
    && std::path::Path::new(path).exists()
  {
    std::fs::remove_file(path)?;
  }
  Ok(())
}

/// If the resolved input file is a `.pdf`, return its path. Returns None
/// for any other extension, or when no file was supplied.
pub(crate) fn resolve_pdf_path(args: &Args) -> Option<String> {
  let file = resolve_input_file(args.file.clone())?;
  let extension = std::path::Path::new(&file)
    .extension()
    .and_then(|ext| ext.to_str())
    .map(|ext| ext.to_lowercase());
  if extension.as_deref() == Some("pdf") { Some(file) } else { None }
}

fn resolve_input_file(args_file: Option<String>) -> Option<String> {
  if let Some(file) = args_file {
    return Some(file);
  }

  let args_vec: Vec<String> = std::env::args().collect();
  if args_vec.len() <= 1 { None } else { args_vec.last().cloned() }
}

fn process_file_input(
  args: &Args,
  file: &str,
  ocr_enabled: bool,
) -> Result<PreparedInput, Box<dyn std::error::Error>> {
  let temp_file = format!("{file}-{}", uuid::Uuid::new_v4());

  let extension = std::path::Path::new(file)
    .extension()
    .and_then(|ext| ext.to_str())
    .map(|ext| ext.to_lowercase());
  let is_pdf = extension.as_deref() == Some("pdf");
  let use_ocr = is_pdf && ocr_enabled;

  let content = if use_ocr {
    Content::verbatim(extract_pdf_text_with_ocr(file)?)
  } else {
    read_content_without_ocr(file, extension.as_deref())?
  };

  let lines = if is_pdf {
    cli_justify::justify_pdf_hybrid(&content.display, args.col)
  } else {
    cli_justify::justify(&content.display, args.col)
  };

  if lines.is_empty() || (lines.len() == 1 && lines[0].trim().is_empty()) {
    eprintln!("Error: No readable content found in file '{file}'");
    eprintln!("The file may be empty, corrupted, or in an unsupported format.");
    std::process::exit(1);
  }

  Ok(PreparedInput {
    lines,
    temp_file: Some(temp_file),
    raw_content: Some(content.identity),
    source_path: Some(file.to_string()),
  })
}

fn extract_pdf_text_with_ocr(
  file: &str,
) -> Result<String, Box<dyn std::error::Error>> {
  cli_pdf_to_text::pdf_to_text_with_bundled_ocr(file)
}

pub(crate) fn read_content_without_ocr(
  file: &str,
  extension: Option<&str>,
) -> Result<Content, Box<dyn std::error::Error>> {
  match extension {
    // Plain text is read as bytes and decoded from whatever encoding it uses
    // (UTF-8/16/32 with or without a BOM, or a legacy code page) — no
    // pandoc round-trip needed.
    Some("txt" | "text") => match std::fs::read(file) {
      Ok(bytes) => {
        let len = bytes.len();
        let content =
          Content::from_bytes(bytes, &format!("text file '{file}'"));
        debug_log_encoding(file, len, &content);
        Ok(content)
      }
      Err(e) => {
        eprintln!("Error:\nUnable to read text file '{file}'\n");
        eprintln!("Details:\n{e}\n");
        std::process::exit(1);
      }
    },
    Some("epub") => match cli_epub_to_text::epub_to_text(file) {
      Ok(content) => Ok(Content::from_converted(content)),
      Err(e) => {
        eprintln!("Error:\nUnable to read EPUB file '{file}'\n");
        eprintln!("Details:\n{e}\n");
        std::process::exit(1);
      }
    },
    Some("pdf") => match cli_pdf_to_text::pdf_to_text(file) {
      Ok(content) => Ok(Content::verbatim(content)),
      Err(e) => {
        eprintln!("Error:\nUnable to read PDF file '{file}'\n");
        eprintln!("Details:\n{e}\n");
        std::process::exit(1);
      }
    },
    _ => read_via_best_effort(file),
  }
}

fn read_via_best_effort(
  file: &str,
) -> Result<Content, Box<dyn std::error::Error>> {
  match pandoc_to_text(file)
    .map(Content::from_converted)
    .or_else(|_| {
      cli_epub_to_text::epub_to_text(file).map(Content::from_converted)
    })
    .or_else(|_| cli_pdf_to_text::pdf_to_text(file).map(Content::verbatim))
  {
    Ok(content) => Ok(content),
    Err(e) => {
      eprintln!("Error:\nUnable to read file '{file}'\n");
      eprintln!("Details:\n{e}\n");

      if which("pandoc").is_none() {
        eprintln!(
          "pandoc not installed!\n\nFor additional formats, install pandoc:\nsudo apt install pandoc\n# scoop install pandoc\n# brew install pandoc"
        );
      }
      std::process::exit(1);
    }
  }
}

#[cfg(test)]
mod tests;
