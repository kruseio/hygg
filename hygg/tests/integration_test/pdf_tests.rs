use std::fs;
use std::path::Path;

use crate::helpers::*;

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

  // `#[cfg(not(feature = "pdf-ocr-bundled"))]` only reflects *this crate's*
  // feature. Under `cargo test --workspace`, Cargo feature unification can
  // still build the invoked `hygg` binary with
  // `cli-pdf-to-text/pdf-ocr-bundled` (hygg-server enables it), in which case
  // `--ocr=on` legitimately succeeds. The no-bundled-OCR error path only
  // exists to assert against when the binary really lacks bundled OCR, so
  // treat a success as "not applicable here".
  if output.status.success() {
    eprintln!(
      "hygg binary was built with bundled OCR (workspace feature unification); \
       skipping the no-bundled-OCR error-path assertion"
    );
    return;
  }

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
