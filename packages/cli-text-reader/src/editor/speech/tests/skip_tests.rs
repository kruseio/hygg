use cli_pdf_to_text::PdfLineKind;

use super::text_kinds;
use crate::editor::speech::build_word_spans;

#[test]
fn build_word_spans_skips_ansi_art_lines() {
  let lines = vec![
    "hello world".to_string(),
    "\x1b[7m▀▀\x1b[0m".to_string(), // ansi art row, skipped
    "again".to_string(),
  ];
  let kinds = vec![PdfLineKind::Text, PdfLineKind::AnsiArt, PdfLineKind::Text];
  let spans = build_word_spans(&lines, &kinds);
  // words: hello, world (line 0) + again (line 2); none on the art line.
  assert_eq!(spans.iter().map(|s| s.line).collect::<Vec<_>>(), vec![0, 0, 2]);
  // "again" abs offset = len("hello world")+1 = 12 (art line adds nothing).
  let again = spans.iter().find(|s| s.line == 2).unwrap();
  assert_eq!(again.abs_start, 12);
}

#[test]
fn build_word_spans_skips_fenced_code_but_keeps_offsets() {
  let lines = vec![
    "Before prose".to_string(),
    "```sh".to_string(),
    "sudo apt install cmake pkgconf".to_string(),
    "```".to_string(),
    "After prose".to_string(),
  ];
  let spans = build_word_spans(&lines, &text_kinds(lines.len()));

  assert_eq!(
    spans.iter().map(|s| s.line).collect::<Vec<_>>(),
    vec![0, 0, 4, 4]
  );
  let after = spans.iter().find(|s| s.line == 4).unwrap();
  let expected_abs = lines[0].len()
    + 1
    + lines[1].len()
    + 1
    + lines[2].len()
    + 1
    + lines[3].len()
    + 1;
  assert_eq!(after.abs_start, expected_abs);
}

#[test]
fn build_word_spans_skips_direct_install_commands() {
  let lines = vec![
    "Install from packages:".to_string(),
    "sudo apt install espeak-ng cmake pkgconf \\".to_string(),
    "  libssl-dev".to_string(),
    "Continue reading here".to_string(),
  ];
  let spans = build_word_spans(&lines, &text_kinds(lines.len()));

  assert_eq!(
    spans.iter().map(|s| s.line).collect::<Vec<_>>(),
    vec![0, 0, 0, 3, 3, 3]
  );
}

#[test]
fn build_word_spans_skips_shell_prompt_command_blocks() {
  let lines = vec![
    "Then, compile and install:".to_string(),
    "  $ tar -zxf git-2.8.0.tar.gz".to_string(),
    "  $ cd git-2.8.0".to_string(),
    "  $ make configure".to_string(),
    "  $ ./configure --prefix=/usr".to_string(),
    "  $ make all doc info".to_string(),
    "  $ sudo make install install-doc install-html install-info".to_string(),
    "After this is done, you can also get Git via Git itself for updates:"
      .to_string(),
  ];
  let spans = build_word_spans(&lines, &text_kinds(lines.len()));

  assert!(spans.iter().any(|span| span.line == 0));
  assert!(spans.iter().any(|span| span.line == 7));
  assert!(
    !spans.iter().any(|span| (1..=6).contains(&span.line)),
    "shell prompt command lines should not be narrated"
  );
}

#[test]
fn build_word_spans_skips_markdown_tables() {
  let lines = vec![
    "Before table".to_string(),
    "| Voice id | Gender | Grade |".to_string(),
    "| --- | --- | --- |".to_string(),
    "| af_heart | female | A |".to_string(),
    "| am_puck | male | C+ |".to_string(),
    "After table".to_string(),
  ];
  let spans = build_word_spans(&lines, &text_kinds(lines.len()));

  assert_eq!(
    spans.iter().map(|s| s.line).collect::<Vec<_>>(),
    vec![0, 0, 5, 5]
  );
}

#[test]
fn build_word_spans_skips_progit_option_tables() {
  let lines = vec![
    "Table 2. Common options to git log".to_string(),
    "Option Description".to_string(),
    "-p Show the patch introduced with each commit.".to_string(),
    "--stat Show statistics for files modified in each commit.".to_string(),
    "--shortstat Display only the changed summary line.".to_string(),
    "After table prose".to_string(),
  ];
  let spans = build_word_spans(&lines, &text_kinds(lines.len()));

  assert_eq!(spans.iter().map(|s| s.line).collect::<Vec<_>>(), vec![5, 5, 5]);
}

#[test]
fn build_word_spans_skips_progit_format_specifier_tables() {
  let lines = vec![
    "Specifier Description".to_string(),
    "%H Commit hash".to_string(),
    "%h Abbreviated commit hash".to_string(),
    "%an Author name".to_string(),
    "After table prose".to_string(),
  ];
  let spans = build_word_spans(&lines, &text_kinds(lines.len()));

  assert_eq!(spans.iter().map(|s| s.line).collect::<Vec<_>>(), vec![4, 4, 4]);
}

#[test]
fn build_word_spans_keeps_regular_dash_bullets() {
  let lines = vec![
    "- Package managers are discussed in prose".to_string(),
    "After bullet".to_string(),
  ];
  let spans = build_word_spans(&lines, &text_kinds(lines.len()));

  assert!(
    spans.iter().any(|span| span.line == 0),
    "ordinary prose bullets should still be narrated"
  );
  assert!(spans.iter().any(|span| span.line == 1));
}
