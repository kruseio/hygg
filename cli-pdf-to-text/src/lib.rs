use hygg_shared::normalize_file_path;
use std::io::{BufWriter, Cursor};

mod heuristics;
mod layout_text_output;
mod pdf_patch;
mod sanitize;
mod stream_recovery;

use heuristics::should_prefer_plaintext_output;
use sanitize::sanitize_layout_text;
use stream_recovery::recover_sparse_code_blocks;

fn load_patched_doc(
  canonical_path: &std::path::Path,
) -> Result<pdf_extract::Document, Box<dyn std::error::Error>> {
  match pdf_patch::patched_pdf_bytes(canonical_path) {
    Ok(bytes) => match pdf_extract::Document::load_mem(&bytes) {
      Ok(doc) => Ok(doc),
      Err(_) => Ok(pdf_extract::Document::load(canonical_path)?),
    },
    Err(_) => Ok(pdf_extract::Document::load(canonical_path)?),
  }
}

fn extract_with_layout_text(
  canonical_path: &std::path::Path,
) -> Result<String, Box<dyn std::error::Error>> {
  let mut output_buf = Vec::new();
  {
    let mut output_file = BufWriter::new(Cursor::new(&mut output_buf));

    let doc = load_patched_doc(canonical_path)?;

    pdf_extract::print_metadata(&doc);

    let mut output = Box::new(layout_text_output::LayoutTextOutput::new(
      &mut output_file as &mut dyn std::io::Write,
    ));

    pdf_extract::output_doc(&doc, output.as_mut())?;
  }

  let text = std::str::from_utf8(&output_buf)
    .map_err(|e| format!("Failed to convert PDF output to UTF-8: {e}"))?
    .to_owned();

  Ok(text)
}

pub fn pdf_to_text(
  pdf_path: &str,
) -> Result<String, Box<dyn std::error::Error>> {
  let canonical_path = normalize_file_path(pdf_path)?;

  #[cfg(target_os = "windows")]
  redirect_stderr::redirect_stdout()?;

  #[allow(unused_assignments)]
  let mut original_fd = -1;

  #[allow(unused_assignments)]
  let mut duplicate_fd = -1;

  #[cfg(not(target_os = "windows"))]
  {
    extern crate libc;

    use std::fs::File;
    use std::io::{self, Write};
    use std::os::fd::AsRawFd;
    use std::os::unix::io::FromRawFd;

    let stdout = io::stdout();
    original_fd = stdout.as_raw_fd();

    duplicate_fd = unsafe { libc::dup(original_fd) };

    let dev_null = File::open("/dev/null")
      .map_err(|e| format!("Failed to open /dev/null: {e}"))?;
    unsafe {
      libc::dup2(dev_null.as_raw_fd(), original_fd);
    }
  }

  let layout_text = extract_with_layout_text(&canonical_path);
  let plaintext_output = pdf_extract::extract_text(&canonical_path);

  #[cfg(target_os = "windows")]
  redirect_stderr::restore_stdout()?;

  #[cfg(not(target_os = "windows"))]
  {
    extern crate libc;

    use std::fs::File;
    use std::io::{self, Write};
    use std::os::fd::AsRawFd;
    use std::os::unix::io::FromRawFd;

    unsafe {
      libc::dup2(duplicate_fd, original_fd);
    }
  }

  let layout_text = layout_text?;
  let mut layout_sanitized = sanitize_layout_text(&layout_text);

  if let Ok(Some(recovered)) =
    recover_sparse_code_blocks(&canonical_path, &layout_sanitized)
  {
    layout_sanitized = recovered;
  }

  if let Ok(plaintext_output) = plaintext_output {
    let plaintext_sanitized = sanitize_layout_text(&plaintext_output);
    if should_prefer_plaintext_output(&layout_sanitized, &plaintext_sanitized) {
      return Ok(plaintext_sanitized);
    }
  }

  Ok(layout_sanitized)
}

#[cfg(test)]
mod tests {
  use std::path::Path;

  use super::{pdf_to_text, should_prefer_plaintext_output};

  #[test]
  fn keeps_layout_when_plaintext_has_no_structural_gain() {
    let layout = concat!(
      "A Heading\n",
      "Some explanatory text.\n",
      "Another paragraph.\n",
    );
    let plaintext = concat!(
      "A Heading\n",
      "Some explanatory text.\n",
      "Another paragraph.\n",
      "Noise line\n",
    );
    assert!(!should_prefer_plaintext_output(layout, plaintext));
  }

  #[test]
  fn keeps_progit_codeblock_lines_in_output() {
    let pdf_path = Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("../test-data/pdf/progit-1-50.pdf");
    if !pdf_path.exists() {
      return;
    }

    let text = pdf_to_text(
      pdf_path.to_str().expect("test PDF path should be valid UTF-8"),
    )
    .expect("expected pdf_to_text to succeed for progit sample");

    for expected in
      ["*.a", "!lib.a", "/TODO", "build/", "doc/*.txt", "doc/**/*.pdf"]
    {
      assert!(
        text.contains(expected),
        "expected recovered codeblock to contain {expected:?}, got excerpt around heading: {:?}",
        text
          .lines()
          .skip_while(|line| {
            !line.contains("Here is another example .gitignore file:")
          })
          .take(40)
          .collect::<Vec<_>>()
      );
    }
  }
}
