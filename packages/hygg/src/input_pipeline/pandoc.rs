//! Conversion of other document formats through pandoc.

use crate::binary_lookup::which;
use hygg_shared::normalize_file_path;
use hygg_shared::text_decode::decode_markup;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub(super) fn pandoc_to_text(
  file_path: &str,
) -> Result<String, Box<dyn std::error::Error>> {
  let Some(pandoc) = which("pandoc") else {
    return Err(
      "pandoc not found. Install with:\nsudo apt install pandoc\n# scoop install pandoc\n# brew install pandoc".into(),
    );
  };

  let canonical_path = normalize_file_path(file_path)?;
  // pandoc only reads UTF-8 (3.x falls back to Latin-1, older releases fail),
  // so a text-based input in another encoding is handed over as a decoded
  // UTF-8 copy. The copy keeps the file name so pandoc still infers the input
  // format from the extension; it is deleted when `utf8_copy` drops.
  let utf8_copy = Utf8Copy::for_pandoc(&canonical_path)?;
  let input = utf8_copy.as_ref().map_or(&canonical_path, |copy| &copy.file);
  let output = Command::new(pandoc)
    .arg("--to=plain")
    .arg("--wrap=none")
    .arg("--")
    .arg(input)
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .output()?;

  if !output.status.success() {
    let stderr = String::from_utf8_lossy(&output.stderr);
    return Err(format!("pandoc failed: {stderr}").into());
  }

  Ok(String::from_utf8(output.stdout)?)
}

/// A UTF-8 transcoded copy of a pandoc input, in a private temporary
/// directory that is removed on drop.
struct Utf8Copy {
  dir: PathBuf,
  file: PathBuf,
}

impl Utf8Copy {
  /// `Some` only for a text-based input that is not UTF-8. Container formats
  /// (docx/odt/epub zips, OLE documents, PDFs) and UTF-8 input (with or without
  /// a BOM, which pandoc strips) are passed to pandoc untouched. Markup is
  /// decoded honouring its own `<meta charset>` / XML `encoding=` declaration.
  fn for_pandoc(path: &Path) -> io::Result<Option<Self>> {
    let bytes = std::fs::read(path)?;
    if std::str::from_utf8(&bytes).is_ok() || is_binary_container(&bytes) {
      return Ok(None);
    }
    let decoded = decode_markup(&bytes);
    if decoded.unsupported() {
      return Ok(None);
    }
    let dir = std::env::temp_dir()
      .join(format!("hygg-pandoc-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir)?;
    let copy = Self {
      file: dir.join(path.file_name().unwrap_or("input".as_ref())),
      dir,
    };
    std::fs::write(&copy.file, decoded.text)?;
    Ok(Some(copy))
  }
}

impl Drop for Utf8Copy {
  fn drop(&mut self) {
    let _ = std::fs::remove_dir_all(&self.dir);
  }
}

fn is_binary_container(bytes: &[u8]) -> bool {
  const MAGIC: [&[u8]; 3] =
    [b"PK\x03\x04", b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1", b"%PDF"];
  MAGIC.iter().any(|magic| bytes.starts_with(magic))
}
