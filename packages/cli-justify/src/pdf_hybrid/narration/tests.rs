use super::{
  is_fence_line, is_markdown_table_row, is_markdown_table_separator,
  looks_like_page_number, looks_like_table_header,
  pdf_hybrid_narration_skip_mask, shell_session_accepts, table_cells,
};

fn mask(lines: &[&str]) -> Vec<bool> {
  pdf_hybrid_narration_skip_mask(
    &lines.iter().map(|line| (*line).to_string()).collect::<Vec<_>>(),
  )
}

#[test]
fn page_number_accepts_only_short_ascii_folios() {
  for accepted in ["7", "Page 42", "page 42", "p. 42", "  1234  "] {
    assert!(looks_like_page_number(accepted), "{accepted:?}");
  }
  for rejected in
    ["", "Page ", "１２", "12345", "Page 12345", "1984 was", "Page VII"]
  {
    assert!(!looks_like_page_number(rejected), "{rejected:?}");
  }
}

#[test]
fn fenced_blocks_and_heredoc_continuations_stop_at_their_boundaries() {
  assert_eq!(
    mask(&["Before", "```rust", "中文 code", "```", "After"]),
    [false, true, true, true, false]
  );
  assert_eq!(mask(&["~~~", "中文", "~~~", "After"]), [true, true, true, false]);
  assert!(is_fence_line("```rust"));
  assert!(is_fence_line("~~~"));
  assert!(!is_fence_line("ordinary prose"));
  let command = mask(&["cat <<EOF", "  中文 payload", "EOF", "After prose"]);
  assert!(command[0]);
  assert!(command[1]);
  assert!(!command[3]);
}

#[test]
fn shell_session_stays_within_its_indent() {
  assert!(!shell_session_accepts("", "  "));
  assert!(!shell_session_accepts("$ prompt", "  "));
  assert!(shell_session_accepts("  output", "  "));
  assert!(shell_session_accepts("    output", "  "));
  assert!(!shell_session_accepts("                output", "  "));
  assert_eq!(
    mask(&["  $ pwd", "  /tmp", "  $ echo 中文", "  中文", "Outside"]),
    [true, true, true, true, false]
  );
}

#[test]
fn markdown_table_requires_a_separator_and_two_cells() {
  assert_eq!(table_cells("| a | | b |"), ["a", "b"]);
  assert!(!is_markdown_table_row("plain"));
  assert!(!is_markdown_table_row("| only |"));
  assert!(is_markdown_table_row("| a | b |"));
  for rejected in ["plain", "| --- |", "| --- | x |", "| --- | :---x |"] {
    assert!(!is_markdown_table_separator(rejected), "{rejected:?}");
  }
  assert!(is_markdown_table_separator("| :--- | ---: |"));
  assert!(!mask(&["| a | b |", "ordinary prose"])[1]);
  assert_eq!(
    mask(&["| a | b |", "|---|---|", "| x | y |", "After"]),
    [true, true, true, false]
  );
}

#[test]
fn table_context_marks_captions_and_headers_but_stops_at_prose() {
  assert!(looks_like_table_header("Option Description"));
  assert!(looks_like_table_header("Type Notes"));
  for rejected in
    ["Description", "a b c d e f Grade", "Option Description.", "Option other"]
  {
    assert!(!looks_like_table_header(rejected), "{rejected:?}");
  }
  assert_eq!(
    mask(&[
      "Prose",
      "Table 2. 中文 data",
      "Option Description",
      "-p Show the patch introduced with each commit.",
      "--stat Show statistics for files modified in each commit.",
      "  and include a wrapped continuation.",
      "After"
    ]),
    [false, true, true, true, true, true, false]
  );
  assert_eq!(
    mask(&["Heading", "", "-p patch", "After"]),
    [false, false, true, false]
  );
}

#[test]
fn aligned_toc_rows_and_continuations_are_skipped_only_while_aligned() {
  assert_eq!(
    mask(&[
      "Chapter 1   中文 title",
      "  continued 中文   12",
      "Other chapter   13",
      "Plain prose"
    ]),
    [true, true, true, false]
  );
  assert_eq!(
    mask(&["Chapter 1   中文 title   12", "Other chapter   13", "Plain prose"]),
    [true, true, false]
  );
  assert_eq!(mask(&["Before", "", "After"]), [false, false, false]);
  assert_eq!(
    mask(&[
      "Chapter 1   中文 title",
      "  more of the title",
      "  final part   12",
      "Ordinary prose"
    ]),
    [true, true, true, false]
  );
  assert_eq!(
    mask(&["Chapter 1   中文 title", "Ordinary prose"]),
    [true, false]
  );
  assert_eq!(mask(&["Title....suffix ... 12"]), [true]);
}

#[test]
fn table_scan_stops_at_blank_lines_and_at_the_end_of_input() {
  assert_eq!(
    mask(&["", "-p Show the patch introduced with each commit."]),
    [false, true]
  );
  assert_eq!(
    mask(&["| a | b |", "|---|---|", "| x | y |"]),
    [true, true, true]
  );
}
