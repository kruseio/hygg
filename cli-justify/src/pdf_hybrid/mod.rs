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
}
