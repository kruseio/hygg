use crate::pdf_hybrid::justify_pdf_hybrid;

#[test]
fn preserves_multi_line_pdf_literal_string_example_layout() {
  // PDF Reference section 3.2.3 shows backslash-continuation by
  // putting one literal string across three source lines. Each
  // source line must stay on its own output line so the reader can
  // see the `\`-at-end-of-line continuation syntax that the example
  // is teaching. The second example sits directly underneath.
  let input = concat!(
    "end-of-line marker following it are not considered part of the string. For example:\n",
    "  ( These \\\n",
    "  two strings \\\n",
    "  are the same . )\n",
    "  ( These two strings are the same . )\n",
  );
  let out = justify_pdf_hybrid(input, 80);

  let opener = out
    .iter()
    .position(|line| line.trim_start() == "( These \\")
    .unwrap_or_else(|| panic!("opener should appear, got: {out:?}"));
  let middle = out
    .iter()
    .position(|line| line.trim_start() == "two strings \\")
    .unwrap_or_else(|| panic!("middle line should appear, got: {out:?}"));
  let closer = out
    .iter()
    .position(|line| line.trim_start() == "are the same . )")
    .unwrap_or_else(|| panic!("closer should appear, got: {out:?}"));
  let second = out
    .iter()
    .position(|line| {
      line.trim_start() == "( These two strings are the same . )"
    })
    .unwrap_or_else(|| panic!("second example should appear, got: {out:?}"));

  assert_eq!(
    middle,
    opener + 1,
    "no blank/join between opener and middle, got: {out:?}"
  );
  assert_eq!(
    closer,
    middle + 1,
    "no blank/join between middle and closer, got: {out:?}"
  );
  assert_eq!(
    second,
    closer + 1,
    "second example follows directly, got: {out:?}"
  );
}

#[test]
fn preserves_pdf_literal_string_example_with_bare_closer() {
  // The PDF Reference also uses `( foo .` on one line and a bare `)`
  // on the next when no backslash continuation is needed. Each line
  // must stay separate so the example reads correctly.
  let input = concat!(
    "For example:\n",
    "  ( This string has an end-of-line at the end of it .\n",
    "  )\n",
    "  ( So does this one .\\n )\n",
  );
  let out = justify_pdf_hybrid(input, 80);

  let opener = out
    .iter()
    .position(|line| line.trim_start().starts_with("( This string"))
    .unwrap_or_else(|| panic!("opener should appear, got: {out:?}"));
  let closer = out
    .iter()
    .position(|line| line.trim_start() == ")")
    .unwrap_or_else(|| panic!("bare closer should appear, got: {out:?}"));
  let second = out
    .iter()
    .position(|line| line.trim_start().starts_with("( So does this one"))
    .unwrap_or_else(|| panic!("second example should appear, got: {out:?}"));

  assert_eq!(
    closer,
    opener + 1,
    "bare closer should not be joined or padded, got: {out:?}"
  );
  assert_eq!(
    second,
    closer + 1,
    "second example follows directly, got: {out:?}"
  );
}
