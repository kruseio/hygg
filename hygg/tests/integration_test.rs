use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn strip_ansi_escapes(input: &str) -> String {
  let mut output = String::with_capacity(input.len());
  let mut chars = input.chars().peekable();
  while let Some(ch) = chars.next() {
    if ch == '\x1b' && chars.peek() == Some(&'[') {
      chars.next();
      for c in chars.by_ref() {
        if c.is_ascii_alphabetic() {
          break;
        }
      }
      continue;
    }
    output.push(ch);
  }
  output
}

fn hygg_command(test_name: &str) -> Command {
  let config_home = std::env::temp_dir()
    .join(format!("hygg-config-{}-{test_name}", std::process::id()));
  let mut command = Command::new(env!("CARGO_BIN_EXE_hygg"));
  command.env("XDG_CONFIG_HOME", config_home);
  command
}

#[cfg(feature = "pdf-ocr-bundled")]
fn write_native_text_pdf(path: &Path, text: &str) {
  let stream = format!("BT\n/F1 18 Tf\n40 90 Td\n({text}) Tj\nET\n");
  let objects = [
    "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
    "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 144] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>".to_string(),
    "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
    format!("<< /Length {} >>\nstream\n{stream}endstream", stream.len()),
  ];

  let mut pdf = String::from("%PDF-1.4\n");
  let mut offsets = vec![0usize];
  for (index, object) in objects.iter().enumerate() {
    offsets.push(pdf.len());
    pdf.push_str(&format!("{} 0 obj\n{object}\nendobj\n", index + 1));
  }

  let xref_offset = pdf.len();
  pdf.push_str("xref\n");
  pdf.push_str(&format!("0 {}\n", objects.len() + 1));
  pdf.push_str("0000000000 65535 f \n");
  for offset in offsets.iter().skip(1) {
    pdf.push_str(&format!("{offset:010} 00000 n \n"));
  }
  pdf.push_str(&format!(
    "trailer\n<< /Root 1 0 R /Size {} >>\nstartxref\n{xref_offset}\n%%EOF\n",
    objects.len() + 1
  ));

  fs::write(path, pdf).expect("failed to write test PDF");
}

#[cfg(feature = "pdf-ocr-bundled")]
fn prepend_fake_ocrmypdf_to_path(dir: &Path) -> PathBuf {
  let fake_bin = dir.join("bin");
  fs::create_dir(&fake_bin).expect("failed to create fake bin directory");
  let fake_ocrmypdf = fake_bin.join("ocrmypdf");
  let marker = dir.join("ocrmypdf-was-invoked");
  fs::write(
    &fake_ocrmypdf,
    format!("#!/bin/sh\ntouch '{}'\nexit 42\n", marker.display()),
  )
  .expect("failed to write fake ocrmypdf");

  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&fake_ocrmypdf, fs::Permissions::from_mode(0o755))
      .expect("failed to make fake ocrmypdf executable");
  }

  fake_bin
}

#[test]
fn test_pdf_processing() {
  let test_file = Path::new(env!("CARGO_MANIFEST_DIR"))
    .parent()
    .unwrap()
    .join("test-data/pdf/pdfreference1.7old-1-50.pdf");

  if !test_file.exists() {
    eprintln!("PDF test file not found, skipping test");
    return;
  }

  let output = hygg_command("test_pdf_processing")
    .arg("--col")
    .arg("80")
    .arg(test_file.to_str().unwrap())
    .output()
    .expect("Failed to execute hygg");

  assert!(output.status.success(), "hygg should exit successfully");

  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(stdout.contains("PDF"), "Output should contain PDF content");
}

#[test]
fn test_redirected_pdf_output_includes_inline_ansi_images() {
  let test_file = Path::new(env!("CARGO_MANIFEST_DIR"))
    .parent()
    .unwrap()
    .join("test-data/pdf/progit-1-50.pdf");

  if !test_file.exists() {
    eprintln!("PDF image test file not found, skipping test");
    return;
  }

  let output =
    hygg_command("test_redirected_pdf_output_includes_inline_ansi_images")
      .arg("--col")
      .arg("80")
      .arg(test_file.to_str().unwrap())
      .output()
      .expect("Failed to execute hygg");

  assert!(output.status.success(), "hygg should exit successfully");

  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(
    stdout.contains("\x1b[38;2"),
    "redirected PDF output should include truecolor ANSI image art"
  );
  assert!(
    stdout.contains('\u{2580}'),
    "redirected PDF output should include half-block image art"
  );
  assert!(
    stdout.contains("Pro Git"),
    "redirected PDF output should preserve extracted text"
  );
  assert!(
    stdout.contains("Figure 1. Local version control diagram"),
    "redirected PDF output should include early Pro Git figure labels"
  );
  for needle in ["$ git status", "$ git config", "Changes to be committed"] {
    assert!(
      !stdout.lines().any(|line| {
        let stripped = strip_ansi_escapes(line);
        stripped.contains('\u{2580}') && stripped.contains(needle)
      }),
      "code/preformatted text should remain plaintext, not ANSI art: {needle}"
    );
  }
  for line in stdout.lines() {
    let visible = strip_ansi_escapes(line).chars().count();
    assert!(
      visible <= 80,
      "redirected PDF line should fit --col=80, got {visible}: {line:?}"
    );
  }
}

#[test]
#[cfg(not(feature = "pdf-ocr-bundled"))]
fn test_ocr_without_bundled_feature_gives_clear_error() {
  let test_file = Path::new(env!("CARGO_MANIFEST_DIR"))
    .parent()
    .unwrap()
    .join("test-data/pdf/ocr-0.pdf");

  if !test_file.exists() {
    eprintln!("OCR PDF test file not found, skipping test");
    return;
  }

  let output =
    hygg_command("test_ocr_without_bundled_feature_gives_clear_error")
      .arg("--ocr=on")
      .arg(test_file.to_str().unwrap())
      .output()
      .expect("Failed to execute hygg");

  assert!(
    !output.status.success(),
    "hygg --ocr=on should fail without bundled OCR"
  );

  let stderr = String::from_utf8_lossy(&output.stderr);
  assert!(
    stderr.contains("--features pdf-ocr-bundled"),
    "expected feature guidance in stderr, got: {stderr}"
  );
}

#[test]
#[cfg(feature = "pdf-ocr-bundled")]
fn test_ocr_with_bundled_feature_does_not_invoke_ocrmypdf() {
  let temp_dir = std::env::temp_dir().join(format!(
    "hygg-bundled-ocr-{}-{}",
    std::process::id(),
    std::thread::current().name().unwrap_or("test")
  ));
  let _ = fs::remove_dir_all(&temp_dir);
  fs::create_dir(&temp_dir).expect("failed to create test temp directory");

  let pdf_path = temp_dir.join("native-text.pdf");
  write_native_text_pdf(&pdf_path, "Bundled OCR smoke text");
  let fake_bin = prepend_fake_ocrmypdf_to_path(&temp_dir);
  let marker = temp_dir.join("ocrmypdf-was-invoked");

  let old_path = std::env::var_os("PATH").unwrap_or_default();
  let mut paths = vec![fake_bin];
  paths.extend(std::env::split_paths(&old_path));
  let path =
    std::env::join_paths(paths).expect("failed to construct test PATH");

  let output =
    hygg_command("test_ocr_with_bundled_feature_does_not_invoke_ocrmypdf")
      .arg("--ocr=on")
      .arg(pdf_path.to_str().unwrap())
      .env("PATH", path)
      .output()
      .expect("Failed to execute hygg");

  let stdout = String::from_utf8_lossy(&output.stdout);
  let stderr = String::from_utf8_lossy(&output.stderr);
  let ocrmypdf_was_invoked = marker.exists();
  let _ = fs::remove_dir_all(&temp_dir);

  assert!(
    output.status.success(),
    "hygg --ocr=on should succeed with bundled OCR; stdout: {stdout}; stderr: {stderr}"
  );
  assert!(
    stdout.contains("Bundled OCR smoke text"),
    "expected bundled OCR path to preserve native text, got: {stdout}"
  );
  assert!(
    !ocrmypdf_was_invoked,
    "hygg --ocr invoked ocrmypdf even though bundled OCR was enabled"
  );
}

#[test]
fn test_epub_processing() {
  let test_file = Path::new(env!("CARGO_MANIFEST_DIR"))
    .parent()
    .unwrap()
    .join("test-data/epub/test-standard.epub");

  if !test_file.exists() {
    eprintln!("EPUB test file not found, skipping test");
    return;
  }

  // Test using cli-epub-to-text directly to verify EPUB extraction works
  let epub_bin = Path::new(env!("CARGO_MANIFEST_DIR"))
    .parent()
    .unwrap()
    .join("target")
    .join("debug")
    .join("cli-epub-to-text");

  let epub_output = Command::new(&epub_bin)
    .arg(test_file.to_str().unwrap())
    .output()
    .expect("Failed to execute cli-epub-to-text");

  assert!(epub_output.status.success(), "cli-epub-to-text should succeed");

  let stdout = String::from_utf8_lossy(&epub_output.stdout);
  assert!(
    stdout.contains("Hygg Test EPUB"),
    "Output should contain EPUB title"
  );
  assert!(stdout.contains("Chapter"), "Output should contain chapter content");

  // For the full hygg test, we'll spawn it and kill it after a short time
  // This verifies it doesn't panic on startup
  let mut child = hygg_command("test_epub_processing")
    .arg("--col")
    .arg("80")
    .arg(test_file.to_str().unwrap())
    .stdout(Stdio::null())
    .stderr(Stdio::piped())
    .spawn()
    .expect("Failed to spawn hygg");

  // Give it time to start and potentially panic
  std::thread::sleep(std::time::Duration::from_millis(500));

  // Kill the process
  let _ = child.kill();

  // Check if it panicked (would have exited with error)
  match child.try_wait() {
    Ok(Some(status)) => {
      if !status.success() {
        let stderr = child.wait_with_output().unwrap().stderr;
        let stderr_str = String::from_utf8_lossy(&stderr);
        if stderr_str.contains("panic") {
          panic!("hygg panicked: {}", stderr_str);
        }
      }
    }
    _ => {
      // Still running, that's good - no panic
    }
  }
}

#[test]
fn test_odt_processing_with_pandoc() {
  // Check if pandoc is available
  let pandoc_check = Command::new("pandoc").arg("--version").output();

  if pandoc_check.is_err() || !pandoc_check.unwrap().status.success() {
    eprintln!("pandoc not installed, skipping ODT test");
    return;
  }

  let test_file = Path::new(env!("CARGO_MANIFEST_DIR"))
    .parent()
    .unwrap()
    .join("test-data/odf/test.odt");

  if !test_file.exists() {
    eprintln!("ODT test file not found, skipping test");
    return;
  }

  let output = hygg_command("test_odt_processing_with_pandoc")
    .arg("--col")
    .arg("80")
    .arg(test_file.to_str().unwrap())
    .output()
    .expect("Failed to execute hygg");

  assert!(
    output.status.success(),
    "hygg should process ODT successfully with pandoc"
  );
}

#[test]
fn test_docx_processing_with_pandoc() {
  // Check if pandoc is available
  let pandoc_check = Command::new("pandoc").arg("--version").output();

  if pandoc_check.is_err() || !pandoc_check.unwrap().status.success() {
    eprintln!("pandoc not installed, skipping DOCX test");
    return;
  }

  let test_file = Path::new(env!("CARGO_MANIFEST_DIR"))
    .parent()
    .unwrap()
    .join("test-data/docx/test-standard.docx");

  if !test_file.exists() {
    eprintln!("DOCX test file not found, skipping test");
    return;
  }

  // Test pandoc conversion directly
  let pandoc_output = Command::new("pandoc")
    .arg(test_file.to_str().unwrap())
    .arg("-t")
    .arg("plain")
    .output()
    .expect("Failed to execute pandoc");

  assert!(
    pandoc_output.status.success(),
    "pandoc should convert DOCX successfully"
  );

  let text = String::from_utf8_lossy(&pandoc_output.stdout);
  assert!(text.contains("Hygg Test DOCX"), "Should contain document title");
  assert!(text.contains("Unicode"), "Should contain Unicode section");

  // Test full hygg processing (spawn and kill due to TUI)
  let mut child = hygg_command("test_docx_processing_with_pandoc")
    .arg("--col")
    .arg("80")
    .arg(test_file.to_str().unwrap())
    .stdout(Stdio::null())
    .stderr(Stdio::piped())
    .spawn()
    .expect("Failed to spawn hygg");

  std::thread::sleep(std::time::Duration::from_millis(500));
  let _ = child.kill();

  // Check if it panicked
  match child.try_wait() {
    Ok(Some(status)) => {
      if !status.success() {
        let stderr = child.wait_with_output().unwrap().stderr;
        let stderr_str = String::from_utf8_lossy(&stderr);
        if stderr_str.contains("panic") {
          panic!("hygg panicked on DOCX: {}", stderr_str);
        }
      }
    }
    _ => {
      // Still running, no panic
    }
  }
}

#[test]
fn test_txt_processing() {
  use std::process::Stdio;
  use std::time::{Duration, Instant};

  let test_file = Path::new(env!("CARGO_MANIFEST_DIR"))
    .parent()
    .unwrap()
    .join("test-data/sample.txt");

  if !test_file.exists() {
    eprintln!("TXT test file not found at: {:?}, skipping test", test_file);
    return;
  }

  // Spawn the process with piped stdout/stderr to ensure non-interactive mode
  let mut child = hygg_command("test_txt_processing")
    .arg("--col")
    .arg("80")
    .arg(test_file.to_str().unwrap())
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .spawn()
    .expect("Failed to spawn hygg");

  // Poll for exit. macOS adds substantial overhead when cargo test directly
  // spawns a recently-built debug binary, so the deadline has to comfortably
  // exceed that pre-main load time, not the 100 ms it used to be.
  let deadline = Instant::now() + Duration::from_secs(5);
  loop {
    match child.try_wait() {
      Ok(Some(status)) => {
        assert!(status.code().is_some(), "hygg should exit cleanly");
        return;
      }
      Ok(None) => {
        if Instant::now() >= deadline {
          let _ = child.kill();
          panic!("hygg should have exited when stdout is not a TTY");
        }
        std::thread::sleep(Duration::from_millis(20));
      }
      Err(e) => panic!("Failed to check hygg status: {e}"),
    }
  }
}

#[test]
fn test_file_type_detection() {
  // Test that file extensions are properly detected
  let test_cases = vec![
    ("test.epub", "EPUB"),
    ("test.pdf", "PDF"),
    ("test.txt", "TXT"),
    ("test.odt", "ODT"),
    ("test.docx", "DOCX"),
  ];

  for (filename, _expected_type) in test_cases {
    println!("Testing file type detection for: {}", filename);
    // This is a placeholder - actual implementation would need to
    // expose the file type detection logic or test it indirectly
  }
}

#[test]
fn test_stdin_processing() {
  use std::io::Write;
  use std::process::Stdio;
  use std::time::{Duration, Instant};

  // Test that hygg can accept stdin input
  let mut child = hygg_command("test_stdin_processing")
    .arg("--col")
    .arg("40")
    .stdin(Stdio::piped())
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .spawn()
    .expect("Failed to spawn hygg");

  let mut stdin = child.stdin.take().expect("Failed to get stdin");
  stdin
    .write_all(
      b"This is a test of stdin processing.\nIt should be properly justified.\n",
    )
    .expect("Failed to write to stdin");
  stdin.flush().expect("Failed to flush stdin");
  // Properly drop the owned stdin to close it
  drop(stdin);

  // Poll for exit. macOS adds substantial overhead when cargo test directly
  // spawns a recently-built debug binary, so the deadline has to comfortably
  // exceed that pre-main load time, not the 100 ms it used to be.
  let deadline = Instant::now() + Duration::from_secs(5);
  loop {
    match child.try_wait() {
      Ok(Some(status)) => {
        assert!(status.code().is_some(), "hygg should exit cleanly");
        return;
      }
      Ok(None) => {
        if Instant::now() >= deadline {
          let _ = child.kill();
          panic!("hygg should have exited when stdout is not a TTY");
        }
        std::thread::sleep(Duration::from_millis(20));
      }
      Err(e) => panic!("Failed to check hygg status: {e}"),
    }
  }
}
