//! # CLI EPUB to Text Converter
//!
//! A Rust library for converting EPUB files to plain text with proper error
//! handling.
//!
//! ## Features
//! - Extract text content from EPUB files
//! - Preserve reading order as defined in the EPUB spine
//! - Convert HTML content to plain text
//! - Comprehensive error handling with custom error types
//! - Unicode support for international content
//!
//! ## Usage
//! ```rust
//! use cli_epub_to_text::epub_to_text;
//!
//! match epub_to_text("path/to/book.epub") {
//!     Ok(text) => println!("Extracted text: {}", text),
//!     Err(e) => eprintln!("Error: {}", e),
//! }
//! ```

use epub::doc::EpubDoc;
use hygg_shared::{PathError, normalize_file_path};
use std::error::Error;
use std::fmt;

/// Custom error type for EPUB processing errors
#[derive(Debug)]
pub enum EpubError {
  FileNotFound(String),
  InvalidEpub(String),
  ResourceNotFound(String),
  HtmlConversion(String),
}

impl fmt::Display for EpubError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      EpubError::FileNotFound(path) => {
        write!(f, "EPUB file not found: {path}")
      }
      EpubError::InvalidEpub(msg) => write!(f, "Invalid EPUB format: {msg}"),
      EpubError::ResourceNotFound(id) => {
        write!(f, "Resource not found in EPUB: {id}")
      }
      EpubError::HtmlConversion(msg) => {
        write!(f, "HTML conversion error: {msg}")
      }
    }
  }
}

impl Error for EpubError {}

impl From<PathError> for EpubError {
  fn from(error: PathError) -> Self {
    match error {
      PathError::FileNotFound(msg) => EpubError::FileNotFound(msg),
      PathError::InvalidPath(msg) => EpubError::InvalidEpub(msg),
      PathError::NotAFile(msg) => EpubError::FileNotFound(msg),
      PathError::IoError(msg) => EpubError::InvalidEpub(msg),
    }
  }
}

/// Convert an EPUB file to plain text
///
/// This function extracts text content from all chapters in an EPUB file,
/// preserving the reading order as defined in the EPUB's spine.
///
/// # Arguments
/// * `file_path` - Path to the EPUB file (can be relative or absolute)
///
/// # Returns
/// * `Ok(String)` - The extracted plain text content with chapters separated by
///   double newlines
/// * `Err(EpubError)` - If the conversion fails for any reason
///
/// # Error Cases
/// * `EpubError::FileNotFound` - The specified file doesn't exist
/// * `EpubError::InvalidEpub` - The file is not a valid EPUB or cannot be
///   parsed
/// * `EpubError::HtmlConversion` - Failed to convert HTML content to plain text
///
/// # Examples
/// ```rust
/// use cli_epub_to_text::epub_to_text;
///
/// // Convert a valid EPUB file
/// match epub_to_text("path/to/book.epub") {
///     Ok(text) => println!("Extracted text: {}", text),
///     Err(e) => eprintln!("Error: {}", e),
/// }
/// ```
pub fn epub_to_text(file_path: &str) -> Result<String, EpubError> {
  let canonical_path = normalize_file_path(file_path)?;

  let mut epub = EpubDoc::new(&canonical_path)
    .map_err(|e| EpubError::InvalidEpub(format!("Failed to open EPUB: {e}")))?;

  extract_spine_text(&mut epub)
}

/// Convert in-memory EPUB bytes to plain text.
///
/// Filesystem-free counterpart of [`epub_to_text`] for environments without
/// path access (e.g. a browser/WASM build importing a `File`). Reads the EPUB
/// from a byte buffer via `EpubDoc::from_reader`; behavior is otherwise
/// identical (spine order, HTML→text conversion, chapter separation).
pub fn epub_bytes_to_text(bytes: &[u8]) -> Result<String, EpubError> {
  let cursor = std::io::Cursor::new(bytes.to_vec());

  let mut epub = EpubDoc::from_reader(cursor)
    .map_err(|e| EpubError::InvalidEpub(format!("Failed to open EPUB: {e}")))?;

  extract_spine_text(&mut epub)
}

/// Walk the EPUB spine in reading order, converting each XHTML resource to
/// plain text and joining chapters with blank lines. Generic over the reader so
/// both the path-based and byte-based entry points share one implementation.
fn extract_spine_text<R: std::io::Read + std::io::Seek>(
  epub: &mut EpubDoc<R>,
) -> Result<String, EpubError> {
  let mut text_parts = Vec::new();

  for spine_item in epub.spine.clone() {
    // Resource not found is skipped silently — some EPUBs reference
    // non-essential resources that don't affect the main content.
    if let Some((xhtml_bytes, _media_type)) =
      epub.get_resource(&spine_item.idref)
    {
      let text =
        html2text::from_read(xhtml_bytes.as_slice(), 110).map_err(|e| {
          EpubError::HtmlConversion(format!(
            "Failed to convert HTML to text for resource '{}': {}",
            spine_item.idref, e
          ))
        })?;

      let trimmed_text = text.trim();
      if !trimmed_text.is_empty() {
        text_parts.push(trimmed_text.to_string());
      }
    }
  }

  if text_parts.is_empty() {
    Err(EpubError::InvalidEpub("No readable content found in EPUB".to_string()))
  } else {
    Ok(text_parts.join("\n\n"))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_epub_error_display() {
    let file_error = EpubError::FileNotFound("test.epub".to_string());
    assert_eq!(format!("{file_error}"), "EPUB file not found: test.epub");

    let invalid_error = EpubError::InvalidEpub("Bad format".to_string());
    assert_eq!(format!("{invalid_error}"), "Invalid EPUB format: Bad format");

    let resource_error = EpubError::ResourceNotFound("chapter1".to_string());
    assert_eq!(
      format!("{resource_error}"),
      "Resource not found in EPUB: chapter1"
    );

    let html_error = EpubError::HtmlConversion("Parse failed".to_string());
    assert_eq!(format!("{html_error}"), "HTML conversion error: Parse failed");
  }

  #[test]
  fn test_epub_error_implements_error_trait() {
    let error: Box<dyn Error> =
      Box::new(EpubError::FileNotFound("test".to_string()));
    assert!(error.source().is_none());
  }

  #[test]
  fn test_file_not_found() {
    let result = epub_to_text("definitely_nonexistent_file.epub");
    assert!(result.is_err());
    match result.unwrap_err() {
      EpubError::FileNotFound(message) => {
        assert!(message.contains("definitely_nonexistent_file.epub"));
        assert!(message.contains("Failed to resolve path"));
      }
      _ => panic!("Expected FileNotFound error"),
    }
  }
}
