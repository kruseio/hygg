use std::path::Path;

use super::{pdf_to_text, should_prefer_plaintext_output};

#[test]
fn keeps_layout_when_plaintext_has_no_structural_gain() {
  let layout =
    concat!("A Heading\n", "Some explanatory text.\n", "Another paragraph.\n",);
  let plaintext = concat!(
    "A Heading\n",
    "Some explanatory text.\n",
    "Another paragraph.\n",
    "Noise line\n",
  );
  assert!(!should_prefer_plaintext_output(layout, plaintext));
}

#[test]
fn keeps_progit_codeblock_lines_in_output() {
  let pdf_path = Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../test-data/pdf/progit-1-50.pdf");
  if !pdf_path.exists() {
    return;
  }

  let text = pdf_to_text(
    pdf_path.to_str().expect("test PDF path should be valid UTF-8"),
  )
  .expect("expected pdf_to_text to succeed for progit sample");

  for expected in
    ["*.a", "!lib.a", "/TODO", "build/", "doc/*.txt", "doc/**/*.pdf"]
  {
    assert!(
      text.contains(expected),
      "expected recovered codeblock to contain {expected:?}, got excerpt around heading: {:?}",
      text
        .lines()
        .skip_while(|line| {
          !line.contains("Here is another example .gitignore file:")
        })
        .take(40)
        .collect::<Vec<_>>()
    );
  }
}
