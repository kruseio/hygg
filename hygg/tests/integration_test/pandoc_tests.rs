use std::path::Path;
use std::process::{Command, Stdio};

use crate::helpers::*;

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
    Ok(Some(status)) if !status.success() => {
      let stderr = child.wait_with_output().unwrap().stderr;
      let stderr_str = String::from_utf8_lossy(&stderr);
      if stderr_str.contains("panic") {
        panic!("hygg panicked on DOCX: {}", stderr_str);
      }
    }
    _ => {
      // Still running, no panic
    }
  }
}
