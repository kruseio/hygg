use crate::pdf_hybrid::justify_pdf_hybrid;

#[test]
fn drops_page_boundary_blank_between_git_log_graph_rows() {
  // pdf_oxide emits a trailing newline at the end of each extracted page,
  // which `text.split('\n')` turns into an empty final token. Without the
  // graph-aware suppression in handle_blank_line, that empty token shows
  // up as a stray blank between page 49's last graph row and page 50's
  // first one when cli-text-reader concatenates them in `flat_lines`.
  let input = concat!(
    "  $ git log --pretty=format:\"%h %s\" --graph\n",
    "  * 2d3acf9 Ignore errors from SIGCHLD on trap\n",
    "  | * 420eac9 Add method for getting the current branch\n",
    "  * | 30e367c Timeout code and tests\n",
    "\n",
  );
  let out = justify_pdf_hybrid(input, 80);
  let last_graph = out
    .iter()
    .position(|line| line.contains("30e367c"))
    .expect("graph row should appear in output");
  assert!(
    out[last_graph + 1..].iter().all(|line| line.is_empty()),
    "no non-empty lines should follow the last graph row, got: {out:?}",
  );
  // And there should be no stray trailing blank either — the graph block
  // should end cleanly with its last row.
  assert_eq!(
    out.last().map(String::as_str),
    Some(out[last_graph].as_str()),
    "graph block should end with its last row, got: {out:?}",
  );
}

#[test]
fn keeps_blank_between_graph_block_and_following_prose() {
  // The fix must not collapse the blank between a graph block and the
  // prose that follows on the same page — that blank is the visual
  // separator readers expect between code and the next paragraph.
  let input = concat!(
    "  * 2d3acf9 Ignore errors from SIGCHLD on trap\n",
    "  * | 30e367c Timeout code and tests\n",
    "\n",
    "This type of output will become more interesting as we go through branching.\n",
  );
  let out = justify_pdf_hybrid(input, 80);
  let last_graph = out
    .iter()
    .position(|line| line.contains("30e367c"))
    .expect("graph row should appear in output");
  let prose_idx = out
    .iter()
    .position(|line| line.contains("This type of output"))
    .expect("prose should appear in output");
  assert!(
    prose_idx > last_graph + 1,
    "prose should follow graph with a gap, got: {out:?}"
  );
  assert!(
    out[last_graph + 1..prose_idx].iter().any(String::is_empty),
    "expected a blank line between graph and prose, got: {out:?}",
  );
}
