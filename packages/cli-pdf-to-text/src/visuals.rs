//! High-fidelity visual extraction for a rich GUI reader: full-resolution
//! embedded images and rasterized table regions, decoded to RGBA8.
//!
//! This is a *render-layer* companion to [`crate::pdf_bytes_to_lines_paged`]:
//! it does NOT change the flattened `(line, kind)` model every hygg client
//! shares, so the width-independent reading anchor is untouched. A frontend
//! maps each visual back onto that model — images by their per-page reading
//! order (the same order the ASCII-art blocks are emitted), tables by their
//! cell text — and paints a crisp raster in place of the monospace fallback.
//! Turning this on or off never moves a reading position or affects any other
//! client.
//!
//! Gated behind `visual-assets` (native only). It rasterizes table regions with
//! pdf_oxide's renderer but does not enable this crate's `pdf-rendering`
//! feature, so page composition stays byte-identical.

use std::cmp::Ordering;
use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::stream::{PdfStream, render_pdf_images_sourced};

/// Cap on a stored raster's largest dimension. Bounds reader memory while
/// staying crisp on a desktop column (which is well under this wide).
const MAX_DIM: u32 = 1600;
/// Render DPI for table regions — enough for sharp text at typical zoom.
const TABLE_DPI: u32 = 150;

/// Which kind of on-page visual a [`PdfVisual`] carries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PdfVisualKind {
  /// An embedded raster image (photo / figure).
  Image,
  /// A rasterized table region (rendered from the source page).
  Table,
}

/// A renderable visual on a page, with enough locating info for a frontend to
/// place it over the matching flattened lines. `rgba` is row-major RGBA8,
/// `width * height * 4` bytes.
#[derive(Clone, Debug)]
pub struct PdfVisual {
  /// 1-based page the visual is on.
  pub page: usize,
  pub kind: PdfVisualKind,
  /// For [`PdfVisualKind::Image`]: this image's index among the page's image
  /// blocks in reading (top-down) order — the exact order
  /// [`crate::pdf_bytes_to_lines_paged`] emits their ASCII-art blocks, so the
  /// Nth block maps to the Nth image. Unused for tables.
  pub ordinal: usize,
  pub width: u32,
  pub height: u32,
  pub rgba: Vec<u8>,
  /// For [`PdfVisualKind::Table`]: the normalized (lowercased, whitespace-
  /// collapsed) cell text, so a frontend can locate the table's rows among the
  /// flattened `Text` lines. `None` for images.
  pub text: Option<String>,
}

impl PdfVisual {
  /// PNG-encode the raster — for a browser `data:` URL, where a raw RGBA buffer
  /// isn't directly usable in an `<img>`. `None` on any encode failure.
  pub fn to_png(&self) -> Option<Vec<u8>> {
    let img =
      image::RgbaImage::from_raw(self.width, self.height, self.rgba.clone())?;
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(out.into_inner())
  }
}

/// A per-page visual extractor over a parsed PDF, so a caller can extract one
/// page at a time — the browser reader uses this to yield between pages and
/// keep the UI responsive on a large document. `col` must match the column the
/// lines were extracted at, so image blocks line up 1:1.
pub struct PdfVisualExtractor {
  stream: PdfStream,
}

impl PdfVisualExtractor {
  pub fn open(pdf_bytes: Vec<u8>) -> Result<Self, Box<dyn std::error::Error>> {
    Ok(Self { stream: PdfStream::open_bytes(pdf_bytes)? })
  }

  pub fn total_pages(&self) -> usize {
    self.stream.total_pages()
  }

  /// One 1-based page's visuals (images then tables). Best-effort and panic-
  /// guarded: a page that fails to parse or render contributes nothing.
  pub fn page(&self, page: usize, col: usize) -> Vec<PdfVisual> {
    if page == 0 || page > self.stream.total_pages() {
      return Vec::new();
    }
    let page_0 = page - 1;
    let mut out = catch_unwind(AssertUnwindSafe(|| {
      page_images(&self.stream, page, page_0, col)
    }))
    .unwrap_or_default();
    out.extend(
      catch_unwind(AssertUnwindSafe(|| {
        page_tables(&self.stream, page, page_0)
      }))
      .unwrap_or_default(),
    );
    out
  }
}

/// Extract every page's high-fidelity visuals from an in-memory PDF, all at
/// once (the native GUI runs this on a worker thread). See
/// [`PdfVisualExtractor`] for the page-at-a-time variant the browser reader
/// streams.
pub fn pdf_bytes_to_visuals(
  pdf_bytes: Vec<u8>,
  col: usize,
) -> Result<Vec<PdfVisual>, Box<dyn std::error::Error>> {
  let ex = PdfVisualExtractor::open(pdf_bytes)?;
  let mut out = Vec::new();
  for page in 1..=ex.total_pages() {
    out.extend(ex.page(page, col));
  }
  Ok(out)
}

/// The page's embedded images as full-resolution RGBA, ordered top-down to
/// match the composed ASCII-art blocks.
fn page_images(
  stream: &PdfStream,
  page: usize,
  page_0: usize,
  col: usize,
) -> Vec<PdfVisual> {
  let images = stream.doc.extract_images(page_0).unwrap_or_default();
  let mut pairs = render_pdf_images_sourced(&stream.doc, page_0, col, &images);
  // Compose emits image blocks sorted by `top` descending (top of page first);
  // mirror that so `ordinal` aligns with the Nth block in the flattened lines.
  pairs
    .sort_by(|a, b| b.0.top.partial_cmp(&a.0.top).unwrap_or(Ordering::Equal));
  pairs
    .into_iter()
    .enumerate()
    .filter_map(|(ordinal, (_, img))| {
      let (width, height, rgba) = downscaled_rgba(img)?;
      Some(PdfVisual {
        page,
        kind: PdfVisualKind::Image,
        ordinal,
        width,
        height,
        rgba,
        text: None,
      })
    })
    .collect()
}

/// The page's tables, each rasterized from its region and tagged with its cell
/// text for locating the rows to cover.
fn page_tables(
  stream: &PdfStream,
  page: usize,
  page_0: usize,
) -> Vec<PdfVisual> {
  let mut tables: Vec<pdf_oxide::structure::table_extractor::Table> = stream
    .doc
    .extract_tables(page_0)
    .unwrap_or_default()
    .into_iter()
    .filter(|t| t.bbox.is_some() && !t.rows.is_empty())
    .collect();
  // Top-down order (largest PDF y = visual top first), matching reading order.
  tables.sort_by(|a, b| {
    let at = a.bbox.as_ref().map(|r| r.bottom()).unwrap_or(0.0);
    let bt = b.bbox.as_ref().map(|r| r.bottom()).unwrap_or(0.0);
    bt.partial_cmp(&at).unwrap_or(Ordering::Equal)
  });

  let opts = pdf_oxide::rendering::RenderOptions::with_dpi(TABLE_DPI);
  let mut out = Vec::new();
  for (ordinal, table) in tables.into_iter().enumerate() {
    let Some(bbox) = table.bbox else { continue };
    if bbox.width <= 1.0 || bbox.height <= 1.0 {
      continue;
    }
    // `bbox.top()` is the region's PDF minimum-y (bottom-left origin), which is
    // exactly the crop origin `render_page_region` expects — mirroring the
    // crate's vector-region path.
    let crop = (bbox.left(), bbox.top(), bbox.width, bbox.height);
    let Ok(rendered) = pdf_oxide::rendering::render_page_region(
      &stream.doc,
      page_0,
      crop,
      &opts,
    ) else {
      continue;
    };
    let Ok(img) = image::load_from_memory(&rendered.data) else {
      continue;
    };
    let Some((width, height, rgba)) = downscaled_rgba(img) else {
      continue;
    };
    // A near-blank crop means the region wasn't where we thought — skip it so
    // the table stays as its (perfectly readable) monospace text instead.
    if is_blank(&rgba) {
      continue;
    }
    out.push(PdfVisual {
      page,
      kind: PdfVisualKind::Table,
      ordinal,
      width,
      height,
      rgba,
      text: Some(normalize_table_text(&table)),
    });
  }
  out
}

/// Convert a decoded image to RGBA8, downscaling so the larger side is at most
/// [`MAX_DIM`]. `None` if the result is degenerate.
fn downscaled_rgba(img: image::DynamicImage) -> Option<(u32, u32, Vec<u8>)> {
  let img = if img.width() > MAX_DIM || img.height() > MAX_DIM {
    img.resize(MAX_DIM, MAX_DIM, image::imageops::FilterType::Triangle)
  } else {
    img
  };
  let rgba = img.to_rgba8();
  let (w, h) = (rgba.width(), rgba.height());
  if w == 0 || h == 0 {
    return None;
  }
  Some((w, h, rgba.into_raw()))
}

/// Whether a raster is almost entirely near-white (an empty crop). Early-exits
/// as soon as enough coloured pixels are seen.
fn is_blank(rgba: &[u8]) -> bool {
  let total = rgba.len() / 4;
  if total == 0 {
    return true;
  }
  let mut coloured = 0usize;
  for px in rgba.chunks_exact(4) {
    if px[0] < 245 || px[1] < 245 || px[2] < 245 {
      coloured += 1;
      // > ~0.5% coloured → not blank.
      if coloured * 200 > total {
        return false;
      }
    }
  }
  true
}

/// Join a table's cell text into a normalized token stream for line matching.
fn normalize_table_text(
  table: &pdf_oxide::structure::table_extractor::Table,
) -> String {
  let mut out = String::new();
  for row in &table.rows {
    for cell in &row.cells {
      for word in cell.text.split_whitespace() {
        if !out.is_empty() {
          out.push(' ');
        }
        out.push_str(&word.to_lowercase());
      }
    }
  }
  out
}
