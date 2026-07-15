use std::path::Path;
use std::process::{Command, Stdio};

use crate::helpers::*;

#[test]
fn test_epub_processing() {
  let test_file = Path::new(env!("CARGO_MANIFEST_DIR"))
    .parent()
    .unwrap()
    .parent()
    .unwrap()
    .join("test-data/epub/test-standard.epub");

  if !test_file.exists() {
    eprintln!("EPUB test file not found, skipping test");
    return;
  }

  // Test using cli-epub-to-text directly to verify EPUB extraction works.
  // Spelled out by path because CARGO_BIN_EXE_* only covers this crate's own
  // binaries, not a sibling's — and target/ is at the workspace root, two
  // levels above packages/hygg.
  let epub_bin = Path::new(env!("CARGO_MANIFEST_DIR"))
    .parent()
    .unwrap()
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
    Ok(Some(status)) if !status.success() => {
      let stderr = child.wait_with_output().unwrap().stderr;
      let stderr_str = String::from_utf8_lossy(&stderr);
      if stderr_str.contains("panic") {
        panic!("hygg panicked: {}", stderr_str);
      }
    }
    _ => {
      // Still running, that's good - no panic
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
