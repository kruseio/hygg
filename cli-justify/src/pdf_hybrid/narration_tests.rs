use super::pdf_hybrid_narration_skip_mask;

fn lines(input: &[&str]) -> Vec<String> {
  input.iter().map(|line| (*line).to_string()).collect()
}

#[test]
fn skips_standalone_page_numbers() {
  let mask = pdf_hybrid_narration_skip_mask(&lines(&[
    "the end of a paragraph.",
    "",
    "42",
    "",
    "The next chapter begins here.",
    "Page 7",
  ]));

  assert_eq!(mask, vec![false, false, true, false, false, true]);
}

#[test]
fn keeps_numeric_prose_and_long_numbers() {
  // A number inside a sentence, and a digit run longer than a folio, must still
  // be narrated — only a short, isolated, digits-only line is a page number.
  let mask = pdf_hybrid_narration_skip_mask(&lines(&[
    "It was released in 1984 to acclaim.",
    "123456",
  ]));

  assert_eq!(mask, vec![false, false]);
}

#[test]
fn skips_shell_prompt_command_blocks() {
  let mask = pdf_hybrid_narration_skip_mask(&lines(&[
    "Then, compile and install:",
    "  $ tar -zxf git-2.8.0.tar.gz",
    "  $ cd git-2.8.0",
    "  $ make configure",
    "  $ ./configure --prefix=/usr",
    "  $ make all doc info",
    "  $ sudo make install install-doc install-html install-info",
    "After this is done, you can also get Git via Git itself for updates:",
  ]));

  assert!(!mask[0]);
  assert!(mask[1..=6].iter().all(|skip| *skip));
  assert!(!mask[7]);
}

#[test]
fn skips_unprompted_command_continuations() {
  let mask = pdf_hybrid_narration_skip_mask(&lines(&[
    "Install from packages:",
    "sudo apt install espeak-ng cmake pkgconf \\",
    "  libssl-dev",
    "Continue reading here",
  ]));

  assert_eq!(mask, vec![false, true, true, false]);
}

#[test]
fn skips_option_table_rows_and_context() {
  let mask = pdf_hybrid_narration_skip_mask(&lines(&[
    "Table 2. Common options to git log",
    "Option Description",
    "-p Show the patch introduced with each commit.",
    "--stat Show statistics for files modified in each commit.",
    "  and include a wrapped continuation.",
    "--shortstat Display only the changed summary line.",
    "After table prose",
  ]));

  assert!(mask[..=5].iter().all(|skip| *skip));
  assert!(!mask[6]);
}

#[test]
fn skips_format_specifier_table_rows() {
  let mask = pdf_hybrid_narration_skip_mask(&lines(&[
    "Specifier Description",
    "%H Commit hash",
    "%h Abbreviated commit hash",
    "%an Author name",
    "After table prose",
  ]));

  assert!(mask[..=3].iter().all(|skip| *skip));
  assert!(!mask[4]);
}

#[test]
fn keeps_regular_bullet_lists_narratable() {
  let mask = pdf_hybrid_narration_skip_mask(&lines(&[
    "- Package managers are discussed in prose",
    "  with a wrapped continuation.",
    "After bullet",
  ]));

  assert_eq!(mask, vec![false, false, false]);
}

#[test]
fn skips_dot_leader_toc_rows() {
  let mask = pdf_hybrid_narration_skip_mask(&lines(&[
    "Before toc",
    "Getting Started . . . . . . . . . . . . . . . . . 1",
    "Installing Git . . . . . . . . . . . . . . . . . . 7",
    "After toc",
  ]));

  assert_eq!(mask, vec![false, true, true, false]);
}

#[test]
fn skips_markdown_tables() {
  let mask = pdf_hybrid_narration_skip_mask(&lines(&[
    "Before table",
    "| Voice id | Gender | Grade |",
    "| --- | --- | --- |",
    "| af_heart | female | A |",
    "| am_puck | male | C+ |",
    "After table",
  ]));

  assert_eq!(mask, vec![false, true, true, true, true, false]);
}
