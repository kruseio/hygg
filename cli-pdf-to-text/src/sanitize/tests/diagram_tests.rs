use crate::sanitize::diagram::strip_diagram_labels;

#[test]
fn strips_figure_label_cluster_above_caption() {
  let input = concat!(
    "         viewer application, such as Acrobat, on any supported platform.\n",
    "                                            Acrobat\n",
    "Macintosh application Windows application\n",
    "                                          Adobe PDF\n",
    "                                             printer\n",
    "\n",
    "  QuickDraw/\n",
    "\n",
    "         CoreGraphics\n",
    "                                                     GDI\n",
    "                                                   PDF\n",
    "\n",
    "                         FIGURE 2.2   Creating PDF files using Acrobat Distiller\n",
    "  2.4 PDF and the PostScript Language\n",
    "         The PDF operators for setting the graphics state and painting graphics\n",
  );

  let output = strip_diagram_labels(input);
  let body =
    "         viewer application, such as Acrobat, on any supported platform.";
  let caption = "                         FIGURE 2.2   Creating PDF files using Acrobat Distiller";
  let next_section = "  2.4 PDF and the PostScript Language";
  let para = "         The PDF operators for setting the graphics state and painting graphics";

  assert!(output.contains(body), "body paragraph should survive: {output:?}");
  assert!(
    output.contains(caption),
    "FIGURE caption should survive: {output:?}"
  );
  assert!(
    output.contains(next_section),
    "section heading should survive: {output:?}"
  );
  assert!(
    output.contains(para),
    "following paragraph should survive: {output:?}"
  );

  for label in [
    "Acrobat\n",
    "Macintosh application Windows application",
    "Adobe PDF\n",
    "printer\n",
    "CoreGraphics\n",
    "GDI\n",
    "PDF\n\n",
  ] {
    assert!(
      !output.contains(label),
      "expected figure label {label:?} to be stripped, got:\n{output}"
    );
  }
}

#[test]
fn strips_unattributed_figure_label_cluster_mid_paragraph() {
  let input = concat!(
    "         (although a few such devices do also\n",
    "                                    PostScript\n",
    "                                page description\n",
    "                                     Acrobat\n",
    "                                        PDF\n",
    "                               Acrobat Distiller\n",
    "\n",
    "         support  PDF  directly).  An  application  printing a PDF document to a\n",
  );

  let output = strip_diagram_labels(input);
  assert!(output.contains("(although a few such devices do also"));
  assert!(output.contains("support  PDF  directly)"));
  for label in
    ["PostScript\n", "page description", "Acrobat\n", "Acrobat Distiller"]
  {
    assert!(!output.contains(label), "expected {label:?} stripped:\n{output}");
  }
}

#[test]
fn preserves_title_page_without_paragraph_above() {
  // Title page is a cluster of labels but has no body paragraph above it,
  // so the heuristic must not strip it.
  let input = concat!(
    "PDF Reference\n",
    "   sixth edition\n",
    "   Adobe® Portable Document Format\n",
    "         Version 1.7\n",
    "        Adobe Systems Incorporated\n",
    "\n",
    "© 1985–2006 Adobe® Systems Incorporated. All rights reserved.\n",
  );

  let output = strip_diagram_labels(input);
  assert!(output.contains("PDF Reference"));
  assert!(output.contains("sixth edition"));
  assert!(output.contains("Adobe® Portable Document Format"));
  assert!(output.contains("Version 1.7"));
  assert!(output.contains("Adobe Systems Incorporated"));
}

#[test]
fn preserves_uniformly_indented_short_list() {
  // A short vertical list at one indent level is not a diagram. The
  // distinct-indents requirement keeps such lists intact.
  let input = concat!(
    "The supported commands are listed below.\n",
    "  cat\n",
    "  ls\n",
    "  cp\n",
    "  mv\n",
    "These commands operate on files.\n",
  );

  let output = strip_diagram_labels(input);
  assert!(output.contains("cat"));
  assert!(output.contains("ls"));
  assert!(output.contains("cp"));
  assert!(output.contains("mv"));
}

#[test]
fn preserves_code_block_recovery_anchor() {
  // The .gitignore example used by stream_recovery looks like a cluster
  // of weak (code-like) labels. With zero strong labels, the cluster must
  // not be stripped or recovery will have no anchor.
  let input = concat!(
    "Here is another example .gitignore file:\n",
    "  *.a\n",
    "  !lib.a\n",
    "  /TODO\n",
    "  build/\n",
    "  doc/*.txt\n",
    "  doc/**/*.pdf\n",
    "More body text follows.\n",
  );

  let output = strip_diagram_labels(input);
  for line in ["*.a", "!lib.a", "/TODO", "build/", "doc/*.txt", "doc/**/*.pdf"]
  {
    assert!(output.contains(line), "code line {line:?} should survive");
  }
}
