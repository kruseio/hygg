use std::path::Path;
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

  let output = Command::new(env!("CARGO_BIN_EXE_hygg"))
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

  let output = Command::new(env!("CARGO_BIN_EXE_hygg"))
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
  for line in stdout.lines() {
    let visible = strip_ansi_escapes(line).chars().count();
    assert!(
      visible <= 80,
      "redirected PDF line should fit --col=80, got {visible}: {line:?}"
    );
  }
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
  let mut child = Command::new(env!("CARGO_BIN_EXE_hygg"))
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

  let output = Command::new(env!("CARGO_BIN_EXE_hygg"))
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
  let mut child = Command::new(env!("CARGO_BIN_EXE_hygg"))
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
  let mut child = Command::new(env!("CARGO_BIN_EXE_hygg"))
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
  let mut child = Command::new(env!("CARGO_BIN_EXE_hygg"))
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
