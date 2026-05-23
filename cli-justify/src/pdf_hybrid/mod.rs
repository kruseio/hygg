mod alignment;
mod code_blocks;
mod engine;
mod engine_handlers;
mod engine_output;
mod figure_labels;
mod page_stream;
mod structure;
mod wrapping;
mod wrapping_plain;

pub use engine::justify_pdf_hybrid;
pub use page_stream::{
  PartialParagraph, PdfPageJustified, justify_pdf_page, justify_pdf_seam,
};

#[cfg(test)]
mod tests {
  use super::justify_pdf_hybrid;

  #[test]
  fn preserves_indent_when_trimmed_content_fits_line_width_exactly() {
    let line = format!(
      "   {}{}{}",
      "Foo Bar".to_string(),
      " ".repeat(31),
      "Baz Qux Quux Corge"
    );
    let out = justify_pdf_hybrid(&line, 80);
    assert!(
      out[0].starts_with("   "),
      "expected indent to be preserved, got: {out:?}"
    );
  }

  #[test]
  fn detects_single_word_section_headings() {
    let input =
      "Some sentence ending here.\nLicense\nThis paragraph starts now.";
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

  #[test]
  fn preserves_plate_listing_as_separate_aligned_entries() {
    let input = concat!(
      "  Plate 3     Lab color space (\u{201c}Lab Color Spaces,\u{201d} page 250)\n",
      "  Plate 4    Color gamuts (\u{201c}Lab Color Spaces,\u{201d} page 250)\n",
      "  Plate 5    Rendering intents (\u{201c}Rendering Intents,\u{201d} page 260)\n",
    );
    let out = justify_pdf_hybrid(input, 80);
    let lines_starting_with_plate: Vec<_> = out
      .iter()
      .filter(|line| line.trim_start().starts_with("Plate "))
      .collect();
    assert!(
      lines_starting_with_plate.len() >= 3,
      "expected each Plate entry on its own line, got: {out:?}"
    );
    for line in &lines_starting_with_plate {
      assert!(
        line.matches("Plate ").count() == 1,
        "expected only one Plate per line, got: {line:?}"
      );
    }
  }

  #[test]
  fn pattern_matches_unknown_label_with_numeric_counter() {
    let input = concat!(
      "  Diagram 1   First diagram (\"Overview\", page 12)\n",
      "  Diagram 2   Second diagram (\"Details\", page 14)\n",
      "  Diagram 3   Third diagram (\"Architecture\", page 18)\n",
    );
    let out = justify_pdf_hybrid(input, 80);
    assert!(
      out
        .iter()
        .filter(|line| line.trim_start().starts_with("Diagram "))
        .count()
        >= 3,
      "expected each Diagram entry preserved on its own line, got: {out:?}"
    );
  }

  #[test]
  fn pattern_does_not_misdetect_contributor_initials_as_toc() {
    // First-name + last-initial rows from a multi-column contributors page
    // must remain a preserved layout row (with original wide spacing
    // intact), not be split into a TOC entry that would absorb the next
    // row.
    let input = concat!(
      "   Akrom K                         Jon Freed                       Sergey Kuznetsov\n",
      "   Alan D. Salewski                Jonathan                        Severino Lorilla Jr\n",
    );
    let out = justify_pdf_hybrid(input, 80);
    let akrom_line = out
      .iter()
      .find(|line| line.contains("Akrom K"))
      .expect("expected an Akrom K line");
    // The wide gaps between columns must survive (multiple consecutive
    // spaces), which would have been collapsed if the row had been
    // reflowed as a paragraph or TOC title.
    assert!(
      akrom_line.contains("Akrom K   "),
      "expected Akrom K row's column gap to be preserved, got: {akrom_line:?}"
    );
    assert!(
      akrom_line.contains("Jon Freed"),
      "expected Jon Freed on same row as Akrom K, got: {akrom_line:?}"
    );
  }

  #[test]
  fn preserves_figure_and_table_aligned_entries() {
    let input = concat!(
      "  Figure 1   First diagram   12\n",
      "  Table 2    First table   42\n",
      "  Figure 3   Second diagram (see page 88)\n",
    );
    let out = justify_pdf_hybrid(input, 80);
    assert!(
      out
        .iter()
        .any(|line| line.contains("Figure 1") && line.contains("First diagram")),
      "expected Figure 1 line preserved, got: {out:?}"
    );
    assert!(
      out
        .iter()
        .any(|line| line.contains("Table 2") && line.contains("First table")),
      "expected Table 2 line preserved, got: {out:?}"
    );
    assert!(
      out.iter().any(
        |line| line.contains("Figure 3") && line.contains("Second diagram")
      ),
      "expected Figure 3 line preserved, got: {out:?}"
    );
  }

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
      row_h_upper < row_h_lower
        && row_h_lower < row_an
        && row_an < row_ad,
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
    assert!(prose_idx > last_row + 1, "prose should follow with a gap, got: {out:?}");
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
    assert!(prose_idx > last_graph + 1, "prose should follow graph with a gap, got: {out:?}");
    assert!(
      out[last_graph + 1..prose_idx].iter().any(String::is_empty),
      "expected a blank line between graph and prose, got: {out:?}",
    );
  }
}
