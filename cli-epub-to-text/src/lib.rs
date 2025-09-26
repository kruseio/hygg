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
  // Use shared path normalization function
  let canonical_path = normalize_file_path(file_path)?;

  let mut epub = EpubDoc::new(&canonical_path)
    .map_err(|e| EpubError::InvalidEpub(format!("Failed to open EPUB: {e}")))?;

  let mut text_parts = Vec::new();

  for spine_item in epub.spine.clone() {
    match epub.get_resource(&spine_item.idref) {
      Some((xhtml_bytes, _media_type)) => {
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
      None => {
        // Resource not found, skip silently as some EPUBs may reference
        // non-essential resources that don't affect the main content
        continue;
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
