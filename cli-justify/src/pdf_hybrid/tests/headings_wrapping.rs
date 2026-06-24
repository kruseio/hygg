use crate::pdf_hybrid::justify_pdf_hybrid;

#[test]
fn preserves_indent_when_trimmed_content_fits_line_width_exactly() {
  let line =
    format!("   {}{}{}", "Foo Bar", " ".repeat(31), "Baz Qux Quux Corge");
  let out = justify_pdf_hybrid(&line, 80);
  assert!(
    out[0].starts_with("   "),
    "expected indent to be preserved, got: {out:?}"
  );
}

#[test]
fn detects_single_word_section_headings() {
  let input = "Some sentence ending here.\nLicense\nThis paragraph starts now.";
  let out = justify_pdf_hybrid(input, 80);
  let license_idx =
    out.iter().position(|line| line.trim() == "License").unwrap();
  let next_idx =
    out.iter().position(|line| line.contains("This paragraph")).unwrap();
  assert!(
    next_idx > license_idx,
    "expected next paragraph after License heading, got: {out:?}"
  );
  assert!(
    !out[license_idx].contains("paragraph"),
    "expected heading to be isolated from following paragraph, got: {out:?}"
  );
}

#[test]
fn detects_question_headings_like_what_is_git() {
  let input = "branching system for non-linear development (see Git Branching).\nWhat is Git?\nSo, what is Git in a nutshell?";
  let out = justify_pdf_hybrid(input, 80);
  let heading_idx =
    out.iter().position(|line| line.trim() == "What is Git?").unwrap();
  assert!(
    out[heading_idx].trim() == "What is Git?",
    "expected What is Git? to stand alone, got: {out:?}"
  );
}

#[test]
fn joins_paragraph_when_blank_line_breaks_a_sentence() {
  let input = "If a file is in the database, it's committed. If it has been\n\nmodified, it is staged.";
  let out = justify_pdf_hybrid(input, 80);
  let joined = out.join("\n");
  assert!(
    joined.contains("If it has been modified"),
    "expected the mid-sentence blank line to be suppressed, got: {out:?}"
  );
}

#[test]
fn keeps_hard_hyphen_in_compound_words() {
  let input = "The result is a platform-\nindependent file.";
  let out = justify_pdf_hybrid(input, 80);
  let joined = out.join(" ");
  assert!(
    joined.contains("platform-independent"),
    "expected compound hyphen to be preserved, got: {out:?}"
  );
}

#[test]
fn dehyphenates_partial_words_split_across_lines() {
  let input = "the text was con-\ntent rich.";
  let out = justify_pdf_hybrid(input, 80);
  let joined = out.join(" ");
  assert!(
    joined.contains("content rich"),
    "expected soft hyphen to be removed, got: {out:?}"
  );
}

#[test]
fn preserves_multi_column_contributor_rows() {
  let input = "   4wk-                            Johannes Schindelin             Sean Head\n   Adam Laflamme                   John Lin                        Sean Jacobs";
  let out = justify_pdf_hybrid(input, 80);
  assert!(
    out.iter().any(|line| line.contains("4wk-")
      && line.contains("Johannes Schindelin")
      && line.contains("Sean Head")),
    "expected first contributor row preserved, got: {out:?}"
  );
  assert!(
    out.iter().any(|line| line.contains("Adam Laflamme")
      && line.contains("John Lin")
      && line.contains("Sean Jacobs")),
    "expected second contributor row preserved, got: {out:?}"
  );
}

#[test]
fn keeps_inline_code_continuation_when_indent_bumps_slightly() {
  let input = "an application reading the data is\n ASCIIHexDecode driven";
  let out = justify_pdf_hybrid(input, 80);
  let joined = out.join(" ");
  assert!(
    joined.contains("is ASCIIHexDecode"),
    "expected slight indent bump to be treated as continuation, got: {out:?}"
  );
}
