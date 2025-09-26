//! # Hygg Shared Utilities
//!
//! This crate contains shared utilities used across the hygg project,
//! including cross-platform path handling and common error types.

use std::path::{Path, PathBuf};

/// Error type for path-related operations
#[derive(Debug)]
pub enum PathError {
  /// The file was not found or could not be resolved
  FileNotFound(String),
  /// The path contains invalid characters
  InvalidPath(String),
  /// The path is not a regular file
  NotAFile(String),
  /// General I/O error occurred
  IoError(String),
}

impl std::fmt::Display for PathError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      PathError::FileNotFound(msg) => write!(f, "File not found: {}", msg),
      PathError::InvalidPath(msg) => write!(f, "Invalid path: {}", msg),
      PathError::NotAFile(msg) => write!(f, "Not a file: {}", msg),
      PathError::IoError(msg) => write!(f, "I/O error: {}", msg),
    }
  }
}

impl std::error::Error for PathError {}

/// Normalizes and validates a file path for cross-platform compatibility
///
/// This function handles:
/// - Resolving relative paths (like `./file.txt` or `.\file.txt` on Windows)
/// - Converting paths to absolute canonical form
/// - Validating that the path points to a regular file
/// - Cross-platform path separator handling
///
/// # Arguments
/// * `file_path` - The file path to normalize (can be relative or absolute)
///
/// # Returns
/// * `Ok(PathBuf)` - The normalized canonical path
/// * `Err(PathError)` - If the path is invalid, doesn't exist, or isn't a file
///
/// # Examples
/// ```rust,no_run
/// use hygg_shared::normalize_file_path;
///
/// // Works with relative paths
/// let path = normalize_file_path("./test.txt")?;
///
/// // Works with absolute paths
/// let path = normalize_file_path("/home/user/document.pdf")?;
///
/// // Works with Windows-style paths
/// let path = normalize_file_path(r".\document.docx")?;
/// # Ok::<(), hygg_shared::PathError>(())
/// ```
pub fn normalize_file_path(file_path: &str) -> Result<PathBuf, PathError> {
  // Check for null bytes
  if file_path.contains('\0') {
    return Err(PathError::InvalidPath(
      "Null bytes not allowed in file path".to_string(),
    ));
  }

  // Check for dangerous characters that could be used for command injection
  // Note: Backslash is valid on Windows, parentheses are common in filenames
  let dangerous_chars = ['|', '&', ';', '`', '$', '<', '>', '\n', '\r'];

  if file_path.chars().any(|c| dangerous_chars.contains(&c)) {
    return Err(PathError::InvalidPath(
      "File path contains dangerous characters".to_string(),
    ));
  }

  // Normalize the path to handle different path separators and resolve relative
  // paths
  let path = Path::new(file_path);

  // Canonicalize the path to resolve . and .. components and convert to
  // absolute path
  let canonical_path = path.canonicalize().map_err(|e| {
    PathError::FileNotFound(format!(
      "Failed to resolve path '{}': {}",
      file_path, e
    ))
  })?;

  // Ensure the file is a regular file
  if !canonical_path.is_file() {
    return Err(PathError::NotAFile("Path is not a regular file".to_string()));
  }

  Ok(canonical_path)
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::fs::File;
  use std::io::Write;

  #[test]
  fn test_normalize_file_path_with_null_bytes() {
    let result = normalize_file_path("test\0file.txt");
    assert!(matches!(result, Err(PathError::InvalidPath(_))));
  }

  #[test]
  fn test_normalize_file_path_with_dangerous_chars() {
    let dangerous_paths = [
      "test|file.txt",
      "test&file.txt",
      "test;file.txt",
      "test`file.txt",
      "test$file.txt",
      "test<file>.txt",
    ];

    for dangerous_path in dangerous_paths {
      let result = normalize_file_path(dangerous_path);
      assert!(
        matches!(result, Err(PathError::InvalidPath(_))),
        "Should reject dangerous path: {}",
        dangerous_path
      );
    }
  }

  #[test]
  fn test_normalize_file_path_nonexistent_file() {
    let result = normalize_file_path("definitely_nonexistent_file.txt");
    assert!(matches!(result, Err(PathError::FileNotFound(_))));
  }

  #[test]
  fn test_normalize_file_path_directory() {
    let result = normalize_file_path(".");
    assert!(matches!(result, Err(PathError::NotAFile(_))));
  }

  #[test]
  fn test_normalize_file_path_success() -> Result<(), Box<dyn std::error::Error>>
  {
    // Create a temporary file for testing
    let temp_file = std::env::temp_dir().join("hygg_test_file.txt");
    {
      let mut file = File::create(&temp_file)?;
      file.write_all(b"test content")?;
    }

    // Test with the temporary file
    let result = normalize_file_path(temp_file.to_str().unwrap());
    assert!(result.is_ok());

    // Clean up
    std::fs::remove_file(&temp_file)?;
    Ok(())
  }

  #[test]
  fn test_path_error_display() {
    let file_error = PathError::FileNotFound("test.txt".to_string());
    assert_eq!(format!("{}", file_error), "File not found: test.txt");

    let invalid_error = PathError::InvalidPath("Bad path".to_string());
    assert_eq!(format!("{}", invalid_error), "Invalid path: Bad path");

    let not_file_error = PathError::NotAFile("is directory".to_string());
    assert_eq!(format!("{}", not_file_error), "Not a file: is directory");

    let io_error = PathError::IoError("permission denied".to_string());
    assert_eq!(format!("{}", io_error), "I/O error: permission denied");
  }
}
