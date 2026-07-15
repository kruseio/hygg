use super::*;
use std::path::Path;

fn normalize_spaces(s: &str) -> String {
  s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn progit_visual_text_reflows_intro_without_fragment_words_or_blank_rows() {
  let pdf_path = Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../test-data/pdf/progit-1-50.pdf");
  if !pdf_path.exists() {
    return;
  }
  let stream = PdfStream::open(pdf_path.to_str().expect("utf-8 path"))
    .expect("PdfStream should open valid test PDF");

  let mut rendered = None;
  for page in 1..=stream.total_pages().min(50) {
    let Some(page) = stream.extract_page_with_images(page, 80) else {
      continue;
    };
    let text = page.lines.join("\n");
    let normalized = normalize_spaces(&text);
    if normalized.contains("more deeply describing what GitHub") {
      rendered = Some(text);
      break;
    }
  }
  let rendered = rendered.expect("expected Pro Git GitHub intro excerpt");
  let lines: Vec<&str> = rendered.lines().collect();
  let start = lines
    .iter()
    .position(|line| normalize_spaces(line).contains("unavoidable. Instead"))
    .expect("expected excerpt start");
  let end = lines
    .iter()
    .position(|line| {
      normalize_spaces(line).contains("to use for your own code")
    })
    .expect("expected excerpt end");
  let excerpt = lines[start..=end].join("\n");

  assert!(
    !excerpt.contains("\n\n"),
    "excerpt should not contain visual-row blank separators:\n{excerpt}"
  );
  assert!(excerpt.contains("knowing"), "expected joined word:\n{excerpt}");
  assert!(excerpt.contains("valuable"), "expected joined word:\n{excerpt}");
  assert!(
    normalize_spaces(&excerpt)
      .contains("more deeply describing what GitHub is"),
    "expected reflowed GitHub clause:\n{excerpt}"
  );
  assert!(
    !excerpt.contains("knowi ng") && !excerpt.contains("valuab le"),
    "same-word fragments should not contain inserted spaces:\n{excerpt}"
  );
}

#[test]
fn progit_image_page_uses_paragraph_reflow_for_visible_text() {
  let pdf_path = Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../test-data/pdf/progit-1-50.pdf");
  if !pdf_path.exists() {
    return;
  }
  let stream = PdfStream::open(pdf_path.to_str().expect("utf-8 path"))
    .expect("PdfStream should open valid test PDF");

  let mut rendered_page = None;
  for page in 1..=stream.total_pages().min(50) {
    let Some(page) = stream.extract_page_with_images(page, 80) else {
      continue;
    };
    let text = page.lines.join("\n");
    let normalized = normalize_spaces(&text);
    if normalized.contains("What is") && normalized.contains("version control")
    {
      rendered_page = Some(page);
      break;
    }
  }
  let page =
    rendered_page.expect("expected Pro Git version-control intro page");
  let normalized_lines: Vec<String> =
    page.lines.iter().map(|line| normalize_spaces(line)).collect();

  assert!(
    !normalized_lines.iter().any(|line| line == "that records"),
    "visual row fragments should not render as standalone prose lines:\n{}",
    page.lines.join("\n")
  );
  assert!(
    normalized_lines.iter().any(|line| {
      line.contains("that records changes to a file or set of files over time")
    }),
    "paragraph reflow should keep the continuation full-width:\n{}",
    page.lines.join("\n")
  );
  assert!(
    normalized_lines
      .iter()
      .any(|line| line.contains("specific versions later")),
    "same paragraph should continue through 'versions later':\n{}",
    page.lines.join("\n")
  );
}

#[test]
fn progit_figure_images_do_not_expose_internal_native_labels() {
  let pdf_path = Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../test-data/pdf/progit-1-50.pdf");
  if !pdf_path.exists() {
    return;
  }
  let stream = PdfStream::open(pdf_path.to_str().expect("utf-8 path"))
    .expect("PdfStream should open valid test PDF");
  let page_0based = 22;
  let rows = positioned_visual_text_rows(&stream.doc, page_0based);
  let images =
    stream.doc.extract_images(page_0based).expect("page should extract images");
  let bbox = images[0].bbox().expect("figure image should have a bbox");
  let region = PdfRegion {
    left: bbox.left(),
    bottom: bbox.top(),
    width: bbox.width,
    height: bbox.height,
  };

  assert!(has_nearby_figure_caption(region, &rows));
  assert!(
    !rows.iter().any(|row| {
      !is_figure_caption(&row.text)
        && visual_text_row_overlaps_region(row, region)
    }),
    "ProGit figure labels are embedded in the image and require OCR"
  );
}

#[cfg(feature = "pdf-ocr-bundled")]
#[test]
fn progit_figure_ocr_overlays_embedded_image_labels() {
  let pdf_path = Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../test-data/pdf/progit-1-50.pdf");
  if !pdf_path.exists() {
    return;
  }
  let stream =
    PdfStream::open_with_bundled_ocr(pdf_path.to_str().expect("utf-8 path"))
      .expect("PdfStream should open valid test PDF");

  let page = stream
    .extract_page_with_images(34, 100)
    .expect("page should render with image rows");
  let rendered = page.lines.join("\n");

  assert!(
    ["Untracked", "Unmodified", "Modified", "Staged"]
      .iter()
      .any(|label| rendered.contains(label)),
    "OCR should recover at least one embedded figure label, got {rendered:?}"
  );
}
