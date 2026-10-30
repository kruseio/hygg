use super::*;
use clap::Parser;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

fn encodings_dir() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test-data/encodings")
}

/// Every fixture with the corpus text it must decode to. Fixtures are named
/// `<encoding>-<lang>.txt`; the corpus sample is `corpus/<lang>.txt`.
fn fixtures() -> Vec<(PathBuf, String)> {
  let mut fixtures: Vec<_> =
    std::fs::read_dir(encodings_dir().join("fixtures"))
      .expect("test-data/encodings/fixtures is part of the repository")
      .map(|entry| entry.unwrap().path())
      .collect();
  fixtures.sort();
  fixtures
    .into_iter()
    .map(|path| {
      let stem = path.file_stem().unwrap().to_str().unwrap();
      let lang = stem.rsplit('-').next().unwrap();
      let corpus = encodings_dir().join("corpus").join(format!("{lang}.txt"));
      (path.clone(), std::fs::read_to_string(corpus).unwrap())
    })
    .collect()
}

/// The reader's `progress::generate_hash` (private to cli-text-reader):
/// `DefaultHasher` over the raw content string.
fn document_hash(text: &str) -> u64 {
  let mut hasher = DefaultHasher::new();
  text.to_string().hash(&mut hasher);
  hasher.finish()
}

fn args(col: usize) -> Args {
  Args::try_parse_from(["hygg", "--col", &col.to_string()]).unwrap()
}

#[test]
fn every_fixture_decodes_to_its_corpus_text() {
  let fixtures = fixtures();
  assert!(fixtures.len() >= 20, "expected the generated fixture set");
  for (path, expected) in fixtures {
    let file = path.to_str().unwrap();
    let content = read_content_without_ocr(file, Some("txt")).unwrap();
    assert_eq!(content.display, expected, "{file}");
    let lines = cli_justify::justify(&content.display, 40);
    assert_eq!(lines, cli_justify::justify(&expected, 40), "{file}");
  }
}

#[test]
fn identity_is_unchanged_for_files_that_opened_before() {
  for (path, _) in fixtures() {
    let file = path.to_str().unwrap();
    let content = read_content_without_ocr(file, Some("txt")).unwrap();
    match std::fs::read_to_string(&path) {
      // Opened before (valid UTF-8, BOM/CRLF included): same identity, so
      // the same document hash and book id as older hygg versions.
      Ok(legacy) => {
        assert_eq!(content.identity, legacy, "{file}");
        assert_eq!(document_hash(&content.identity), document_hash(&legacy));
      }
      // Failed to open before: no saved state; identity is the clean text.
      Err(_) => assert_eq!(content.identity, content.display, "{file}"),
    }
  }
}

#[test]
fn utf8_bom_document_keeps_its_hash_but_not_its_bom() {
  let path = encodings_dir().join("fixtures/utf8-bom-english.txt");
  let content =
    read_content_without_ocr(path.to_str().unwrap(), Some("txt")).unwrap();
  assert!(content.identity.starts_with('\u{FEFF}'));
  assert!(!content.display.starts_with('\u{FEFF}'));
  assert_eq!(content.identity.trim_start_matches('\u{FEFF}'), content.display);
}

#[test]
fn piped_utf16_reaches_the_reader_decoded() {
  // What `printf '\xff\xfeH\x00i\x00...' | hygg` hands the pipeline.
  let text = "¿Qué tal? Ünïcödé über alles.\r\n\r\nSecond paragraph: 日本語.";
  let mut bytes = vec![0xFF, 0xFE];
  bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));

  let identity = stdin_identity(bytes).expect("non-empty stdin is input");
  let prepared =
    prepare_input(&args(40), Some(identity.clone()), false).unwrap();
  let clean = "¿Qué tal? Ünïcödé über alles.\n\nSecond paragraph: 日本語.";
  assert_eq!(prepared.lines, cli_justify::justify(clean, 40));
  assert_eq!(prepared.raw_content.as_deref(), Some(clean));
  assert_eq!(identity, clean);
}

#[test]
fn piped_utf8_keeps_its_exact_identity() {
  let raw = "\u{FEFF}line one\r\n\r\nline two\r\n";
  let identity = stdin_identity(raw.as_bytes().to_vec()).unwrap();
  assert_eq!(identity, raw);
  let prepared = prepare_input(&args(80), Some(identity), false).unwrap();
  assert_eq!(prepared.raw_content.as_deref(), Some(raw));
  assert_eq!(
    prepared.lines,
    cli_justify::justify("line one\n\nline two\n", 80)
  );
}

#[test]
fn empty_stdin_is_no_input() {
  assert_eq!(stdin_identity(Vec::new()), None);
}

#[test]
fn undecodable_encodings_get_a_clear_error() {
  let decoded = hygg_shared::text_decode::decode_text(b"\x0E\xFE\xFFhello");
  let message = content::unsupported_encoding_error(&decoded, "stdin").unwrap();
  assert!(message.contains("SCSU"), "{message}");
  let plain = hygg_shared::text_decode::decode_text(b"hello");
  assert!(content::unsupported_encoding_error(&plain, "stdin").is_none());
}

#[test]
fn pandoc_reads_non_utf8_markup_through_a_utf8_copy() {
  if which("pandoc").is_none() {
    eprintln!("pandoc not installed; skipping");
    return;
  }
  let dir = std::env::temp_dir()
    .join(format!("hygg-encoding-test-{}", uuid::Uuid::new_v4()));
  std::fs::create_dir(&dir).unwrap();

  // Windows-1252: “ ” — é ï are 0x93 0x94 0x97 0xE9 0xEF.
  let md =
    b"# R\xE9sum\xE9\n\n\x93Smart quotes\x94 and a na\xEFve caf\xE9 \x97 \
             in cp1252.\n";
  let md_path = dir.join("notes.md");
  std::fs::write(&md_path, md).unwrap();
  let text = pandoc::pandoc_to_text(md_path.to_str().unwrap()).unwrap();
  assert!(text.contains("\u{201C}Smart quotes\u{201D}"), "{text}");
  assert!(text.contains("naïve café — in cp1252"), "{text}");

  let html = "<html><head><meta charset=\"utf-16\"></head>\
              <body><p>Größe: 日本語</p></body></html>";
  let mut bytes = vec![0xFF, 0xFE];
  bytes.extend(html.encode_utf16().flat_map(u16::to_le_bytes));
  let html_path = dir.join("page.html");
  std::fs::write(&html_path, bytes).unwrap();
  let text = pandoc::pandoc_to_text(html_path.to_str().unwrap()).unwrap();
  assert!(text.contains("Größe: 日本語"), "{text}");

  // The transcoded copies are cleaned up.
  let leftovers = std::fs::read_dir(std::env::temp_dir())
    .unwrap()
    .filter_map(|e| e.ok())
    .filter(|e| e.file_name().to_string_lossy().starts_with("hygg-pandoc-"))
    .filter(|e| e.path().join("notes.md").exists())
    .count();
  assert_eq!(leftovers, 0);
  std::fs::remove_dir_all(dir).unwrap();
}
