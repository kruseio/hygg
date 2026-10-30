//! End-to-end text-encoding checks through the real `hygg` binary. With stdout
//! piped, hygg runs the full input pipeline (read, decode, justify) and prints
//! the justified lines instead of opening the reader, so the output can be
//! compared with the UTF-8 corpus the fixtures were generated from
//! (tools/gen-encoding-fixtures.py).

use std::io::Write;
use std::path::PathBuf;
use std::process::{Output, Stdio};

use crate::helpers::hygg_command;

fn encodings_dir() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test-data/encodings")
}

/// Justification moves whitespace around and hard-splits long runs (CJK, Thai)
/// across lines; the non-whitespace characters, in order, are what must match.
fn ink(text: &str) -> String {
  text.chars().filter(|c| !c.is_whitespace()).collect()
}

fn run_hygg(test: &str, args: &[&str], stdin: Option<&[u8]>) -> Output {
  let mut child = hygg_command(test)
    .args(args)
    .stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() })
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .expect("failed to spawn hygg");
  if let Some(bytes) = stdin {
    child.stdin.take().unwrap().write_all(bytes).unwrap();
  }
  child.wait_with_output().unwrap()
}

#[test]
fn every_encoding_fixture_opens_with_the_right_text() {
  let mut fixtures: Vec<_> =
    std::fs::read_dir(encodings_dir().join("fixtures"))
      .expect("test-data/encodings/fixtures is part of the repository")
      .map(|entry| entry.unwrap().path())
      .collect();
  fixtures.sort();
  assert!(fixtures.len() >= 20, "expected the generated fixture set");

  let mut failures = Vec::new();
  for fixture in &fixtures {
    let name = fixture.file_name().unwrap().to_str().unwrap();
    let lang = fixture.file_stem().unwrap().to_str().unwrap();
    let lang = lang.rsplit('-').next().unwrap();
    let corpus = encodings_dir().join("corpus").join(format!("{lang}.txt"));
    let expected = std::fs::read_to_string(corpus).unwrap();

    let output = run_hygg(
      "encoding_fixture",
      &["--col", "40", fixture.to_str().unwrap()],
      None,
    );
    let stdout = String::from_utf8(output.stdout).expect("hygg prints UTF-8");
    if !output.status.success() {
      failures.push(format!("{name}: exit {:?}", output.status.code()));
    } else if ink(&stdout) != ink(&expected) {
      failures.push(format!("{name}: text differs"));
    } else if stdout.contains('\u{FEFF}') || stdout.contains('\r') {
      failures.push(format!("{name}: BOM or CR leaked into the output"));
    }
  }
  assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn piped_utf16_with_bom_is_read_not_ignored() {
  // Equivalent to `printf '\xff\xfe...' | hygg --col 30` (Notepad's
  // "Unicode" output): previously read_to_string failed and the input was
  // silently treated as absent.
  let text = "¿Dónde está el niño?\r\n\r\nΚαλημέρα κόσμε! Привет, мир!";
  let mut bytes = vec![0xFF, 0xFE];
  bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));

  let output = run_hygg("encoding_stdin_utf16", &["--col", "30"], Some(&bytes));
  assert!(output.status.success());
  let stdout = String::from_utf8(output.stdout).unwrap();
  assert_eq!(ink(&stdout), ink(text));
  assert!(stdout.contains("\n\n"), "paragraphs survive: {stdout:?}");
}

#[test]
fn piped_legacy_encoding_is_detected() {
  let text = "Съешь же ещё этих мягких французских булок да выпей чаю. \
              Москва - столица России и крупнейший город страны.\n";
  let koi8: Vec<u8> = {
    let path = encodings_dir().join("fixtures/koi8r-russian.txt");
    std::fs::read(path).unwrap()
  };
  let output = run_hygg("encoding_stdin_koi8", &["--col", "50"], Some(&koi8));
  assert!(output.status.success());
  let stdout = String::from_utf8(output.stdout).unwrap();
  assert!(ink(&stdout).contains(&ink(text)[..40]), "{stdout}");
}

#[test]
fn undecodable_encoding_fails_with_a_clear_message() {
  let dir = std::env::temp_dir()
    .join(format!("hygg-encoding-it-{}", std::process::id()));
  std::fs::create_dir_all(&dir).unwrap();
  let path = dir.join("scsu.txt");
  std::fs::write(&path, b"\x0E\xFE\xFFhello").unwrap();

  let output =
    run_hygg("encoding_scsu", &["--col", "40", path.to_str().unwrap()], None);
  let _ = std::fs::remove_dir_all(&dir);
  assert_eq!(output.status.code(), Some(1));
  let stderr = String::from_utf8_lossy(&output.stderr);
  assert!(stderr.contains("SCSU"), "{stderr}");
  assert!(!stderr.contains("panicked"), "{stderr}");
}
