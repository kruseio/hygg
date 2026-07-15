use crate::pdf_hybrid::justify_pdf_hybrid;

#[test]
fn renders_options_table_one_row_per_line() {
  // progit's "Table 2. Common options to git log" page: each `-flag` row
  // should stand alone (with its description wrapped under it), instead
  // of collapsing into one flowed paragraph.
  let input = concat!(
    "Some prose ending here.\n",
    "Table 2. Common options to git log\n",
    " Option Description\n",
    " -p Show the patch introduced with each commit.\n",
    " --stat Show statistics for files modified in each commit.\n",
    " --shortstat Display only the changed/insertions/deletions line from the --stat command.\n",
  );
  let out = justify_pdf_hybrid(input, 80);
  let row_p = out
    .iter()
    .position(|line| line.trim_start().starts_with("-p "))
    .expect("-p row should appear");
  let row_stat = out
    .iter()
    .position(|line| line.trim_start().starts_with("--stat "))
    .expect("--stat row should appear");
  let row_shortstat = out
    .iter()
    .position(|line| line.trim_start().starts_with("--shortstat "))
    .expect("--shortstat row should appear");
  assert!(
    row_p < row_stat && row_stat < row_shortstat,
    "option rows should appear in order, got: {out:?}",
  );
  // No row should be glued onto the next via "  -p ... --stat ..." style
  // flowing prose.
  for row in &out[row_p..=row_shortstat] {
    let trimmed = row.trim_start();
    if trimmed.starts_with('-') {
      assert!(
        trimmed.matches(" -p ").count() == 0
          && trimmed.matches(" --stat ").count() == 0,
        "row should not contain a second option marker, got: {row:?}",
      );
    }
  }
}

#[test]
fn renders_format_specifier_table_one_row_per_line() {
  // progit's "Table 1. Useful specifiers for git log --pretty=format":
  // each %X row should stand alone instead of flowing as one long
  // paragraph the way the options table did before the marker fixes.
  let input = concat!(
    " %H Commit hash\n",
    " %h Abbreviated commit hash\n",
    " %an Author name\n",
    " %ad Author date (format respects the --date=option)\n",
  );
  let out = justify_pdf_hybrid(input, 80);
  let row_h_upper = out
    .iter()
    .position(|line| line.trim_start().starts_with("%H "))
    .expect("%H row should appear");
  let row_h_lower = out
    .iter()
    .position(|line| line.trim_start().starts_with("%h "))
    .expect("%h row should appear");
  let row_an = out
    .iter()
    .position(|line| line.trim_start().starts_with("%an "))
    .expect("%an row should appear");
  let row_ad = out
    .iter()
    .position(|line| line.trim_start().starts_with("%ad "))
    .expect("%ad row should appear");
  assert!(
    row_h_upper < row_h_lower && row_h_lower < row_an && row_an < row_ad,
    "specifier rows should appear in order, got: {out:?}",
  );
  for row in &out[row_h_upper..=row_ad] {
    // No row should contain a second specifier marker (would indicate
    // flowing).
    let body = row.trim_start();
    if body.starts_with('%') {
      assert!(
        body.matches(" %").count() == 0,
        "row should not flow into the next %X, got: {row:?}",
      );
    }
  }
}

#[test]
fn inserts_blank_line_between_table_end_and_following_prose() {
  // After the last row of an option/spec table, prose typically resumes
  // without an input blank line — the PDF marks the boundary with extra
  // leading instead. Insert an explicit blank so the table and the next
  // sentence don't run together.
  let input = concat!(
    " %ar Author date, relative\n",
    " %s Subject\n",
    "You may be wondering what the difference is between author and committer.\n",
  );
  let out = justify_pdf_hybrid(input, 80);
  let last_row = out
    .iter()
    .position(|line| line.trim_start().starts_with("%s "))
    .expect("%s row should appear");
  let prose_idx = out
    .iter()
    .position(|line| line.contains("You may be wondering"))
    .expect("prose should appear");
  assert!(
    prose_idx > last_row + 1,
    "prose should follow with a gap, got: {out:?}"
  );
  assert!(
    out[last_row + 1..prose_idx].iter().any(String::is_empty),
    "expected a blank line between the table and the prose, got: {out:?}",
  );
}

#[test]
fn separates_table_caption_from_preceding_prose() {
  // The "Table N. Title" caption sits on its own typographic line in the
  // PDF but pdf_extract drops that signal; without forcing a paragraph
  // break we collapse the caption into the trailing sentence above.
  let input = concat!(
    "Some prose ending without a period\n",
    "Table 2. Common options to git log\n",
    " Option Description\n",
  );
  let out = justify_pdf_hybrid(input, 80);
  let caption_idx = out
    .iter()
    .position(|line| line.contains("Table 2. Common options"))
    .expect("caption should appear");
  let caption_line = &out[caption_idx];
  assert!(
    caption_line.trim_start().starts_with("Table 2."),
    "caption should start on its own line, got: {caption_line:?}",
  );
  assert!(
    !caption_line.contains("prose ending"),
    "caption should not be glued to the preceding prose, got: {caption_line:?}",
  );
  // And the prose paragraph should be visually separated from the
  // caption by a blank line — captions are paragraph-level labels for
  // the figure that follows.
  assert!(
    caption_idx >= 1 && out[caption_idx - 1].is_empty(),
    "expected a blank line immediately before the caption, got: {out:?}",
  );
}
