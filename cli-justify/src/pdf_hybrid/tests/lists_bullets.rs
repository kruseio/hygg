use crate::pdf_hybrid::justify_pdf_hybrid;

#[test]
fn renders_list_of_plates_without_blank_separators_or_misplaced_wraps() {
  // Page 11 of pdfreference1.7old.pdf lists the color plates as a
  // sequence of `Plate N …` entries, some with a wrap continuation
  // (`        241)`). The engine used to split each Plate from the
  // next with a blank line (caption-after-caption padding) and pull
  // the `        241)` tail out into its own code block (symbol
  // density flagged the single-paren wrap fragment as code).
  let input = concat!(
    "Plate 1 Additive and subtractive color (Section 4.5.3, \"Device Color Spaces,\" page\n",
    "        241)\n",
    "Plate 2 Uncalibrated color (Section 4.5.4, \"CIE-Based Color Spaces,\" page 244)\n",
    "Plate 3 Lab color space (\"Lab Color Spaces,\" page 250)\n",
  );
  let out = justify_pdf_hybrid(input, 80);

  let plate1_idx = out
    .iter()
    .position(|line| line.starts_with("Plate 1 "))
    .expect("Plate 1 should appear");
  let plate2_idx = out
    .iter()
    .position(|line| line.starts_with("Plate 2 "))
    .expect("Plate 2 should appear");
  let plate3_idx = out
    .iter()
    .position(|line| line.starts_with("Plate 3 "))
    .expect("Plate 3 should appear");

  assert!(
    !out[plate1_idx..plate2_idx].iter().any(String::is_empty),
    "no blank should separate Plate 1's wrapped block from Plate 2, got: {out:?}"
  );
  assert!(
    !out[plate2_idx..plate3_idx].iter().any(String::is_empty),
    "no blank should separate Plate 2 from Plate 3, got: {out:?}"
  );
  let p1_block = out[plate1_idx..plate2_idx].join(" ");
  assert!(
    p1_block.contains("page 241)"),
    "Plate 1 tail `page 241)` should reflow on a continuation line, got: {out:?}"
  );
  // Caption paragraphs use plain wrap: no extra inter-word spaces.
  assert!(
    !out[plate1_idx].contains("Plate  1"),
    "Plate captions should not be justified with extra spaces, got: {:?}",
    out[plate1_idx]
  );
}

#[test]
fn collapses_page_break_blank_between_sibling_bullets() {
  // Section 1.1 of the PDF Reference lists the chapter overview as a
  // single bulleted list that spans a page boundary. pdf_oxide emits
  // a blank between Chapter 7 (last on one page) and Chapter 8 (first
  // on the next), splitting the list. The blank must collapse so the
  // list reads as one continuous block.
  let input = concat!(
    "The rest of the book is organized as follows:\n",
    "\n",
    "• Chapter 2, Overview.\n",
    "  More chapter 2 content.\n",
    "• Chapter 7, Transparency, last item on the page.\n",
    "\n",
    "• Chapter 8, Interactive Features, first item on next page.\n",
    "  More chapter 8 content.\n",
    "• Chapter 9, Multimedia Features.\n",
  );
  let out = justify_pdf_hybrid(input, 80);

  let ch7_idx = out
    .iter()
    .position(|line| line.starts_with("• Chapter 7"))
    .expect("Chapter 7 should appear");
  let ch8_idx = out
    .iter()
    .position(|line| line.contains("Chapter") && line.contains("8,"))
    .expect("Chapter 8 should appear");
  let ch9_idx = out
    .iter()
    .position(|line| line.contains("Chapter 9"))
    .expect("Chapter 9 should appear");

  assert!(
    !out[ch7_idx..ch8_idx].iter().any(String::is_empty),
    "page-break blank between Chapter 7 and Chapter 8 should collapse, got: {out:?}"
  );
  assert!(
    !out[ch8_idx..ch9_idx].iter().any(String::is_empty),
    "no spurious blank between Chapter 8 and Chapter 9, got: {out:?}"
  );
}

#[test]
fn preserves_blank_between_bullet_list_and_following_prose() {
  // The collapse logic must not remove a blank that genuinely
  // terminates a list and introduces a fresh prose paragraph.
  let input =
    concat!("• First.\n", "• Second.\n", "\n", "Now back to prose.\n",);
  let out = justify_pdf_hybrid(input, 80);

  let second_idx = out
    .iter()
    .position(|line| line.starts_with("• Second"))
    .expect("second bullet should appear");
  let prose_idx = out
    .iter()
    .position(|line| line.starts_with("Now back"))
    .expect("prose should appear");

  assert!(
    out[second_idx + 1..prose_idx].iter().any(String::is_empty),
    "blank between list end and prose paragraph should remain, got: {out:?}"
  );
}

#[test]
fn collapses_page_break_blank_between_sibling_captions() {
  // The Plates section lists 20+ caption entries that span page
  // breaks. The page-break blank between two adjacent captions
  // (here Plate 3 and Plate 4) must collapse so the list reads as
  // one continuous block.
  let input = concat!(
    "Plate 2 Uncalibrated color (page 244)\n",
    "Plate 3 Lab color space (page 250)\n",
    "\n",
    "Plate 4 Color gamuts (page 250)\n",
    "Plate 5 Rendering intents (page 260)\n",
  );
  let out = justify_pdf_hybrid(input, 80);

  let p3_idx = out
    .iter()
    .position(|line| line.starts_with("Plate 3 "))
    .expect("Plate 3 should appear");
  let p4_idx = out
    .iter()
    .position(|line| line.starts_with("Plate 4 "))
    .expect("Plate 4 should appear");

  assert!(
    !out[p3_idx..p4_idx].iter().any(String::is_empty),
    "page-break blank between Plate 3 and Plate 4 should collapse, got: {out:?}"
  );
}

#[test]
fn collapses_blank_between_caption_with_wide_gap_and_next_caption() {
  // Plate 3 in the real PDF Reference has wide internal spacing
  // (`Plate 3     Lab color space …`) so the engine routes it
  // through the preserved-layout path, which clears the pending
  // block. Without context-aware detection, the next caption is
  // treated as a "prose → caption" transition and a spurious blank
  // gets inserted between Plate 3 and Plate 4.
  let input = concat!(
    "Plate 2 Uncalibrated color (page 244)\n",
    "Plate 3     Lab color space (page 250)\n",
    "Plate 4 Color gamuts (page 250)\n",
  );
  let out = justify_pdf_hybrid(input, 80);

  let p3_idx = out
    .iter()
    .position(|line| line.starts_with("Plate 3"))
    .expect("Plate 3 should appear");
  let p4_idx = out
    .iter()
    .position(|line| line.starts_with("Plate 4"))
    .expect("Plate 4 should appear");

  assert!(
    !out[p3_idx..p4_idx].iter().any(String::is_empty),
    "no spurious blank should appear between Plate 3 (preserved-layout) and Plate 4, got: {out:?}"
  );
}
