//! Reader image assets: full-resolution figures and rasterized tables mapped
//! onto the open document's flattened lines, for the "Images" render mode.
//!
//! A pure render-layer overlay — it never touches the `Book`'s lines, kinds, or
//! page anchors. A table asset hides its text rows visually while they still
//! count toward the reading anchor, so progress stays identical to every other
//! client (which shows the text). Extraction + placement live in
//! `cli_pdf_to_text` (shared with the native GUI); here each raster is PNG-
//! encoded to a `data:` URL an `<img>` can show.

use cli_pdf_to_text::{PdfVisualExtractor, place_visuals};

use crate::model::{Book, LineKind};
use crate::storage;

/// A raster to draw over a contiguous run of document lines. `line_count` lines
/// (each one text row tall) are replaced by the image, so the document's total
/// height — and thus the scroll/anchor math — is unchanged.
#[derive(Clone, Debug, PartialEq)]
pub struct ImageAsset {
  pub line_start: usize,
  pub line_count: usize,
  pub data_url: String,
}

/// Parse the PDF for book `id` from its cached source bytes, returning a live
/// per-page visual extractor the reader keeps alive and queries as it scrolls.
/// `None` for non-PDFs, missing content, or a parse failure. pdf_oxide parses
/// lazily, so this is fast even on a large document.
pub async fn open(id: String) -> Option<PdfVisualExtractor> {
  let Ok(Some(book)) = storage::get_book(&id).await else {
    return None;
  };
  if book.format != "pdf" {
    return None;
  }
  let bytes = storage::get_blob(&id).await?;
  PdfVisualExtractor::open(bytes).ok()
}

/// Extract, place, and PNG-encode one 1-based `page`'s visuals into
/// `<img>`-ready assets. CPU-heavy (decode + rasterize + encode) and wasm is
/// single-threaded, so the reader calls this only for pages reaching the
/// viewport and yields between them to keep scrolling/input live.
pub fn page_assets(
  ex: &PdfVisualExtractor,
  book: &Book,
  page: usize,
) -> Vec<ImageAsset> {
  let visuals = ex.page(page, book.col.max(1));
  if visuals.is_empty() {
    return Vec::new();
  }
  let is_image = |i: usize| matches!(book.kinds.get(i), Some(LineKind::Ansi));
  place_visuals(&visuals, &book.lines, is_image, &book.page_starts)
    .into_iter()
    .filter_map(|p| {
      let png = visuals[p.visual].to_png()?;
      Some(ImageAsset {
        line_start: p.line_start,
        line_count: p.line_count,
        data_url: format!("data:image/png;base64,{}", base64(&png)),
      })
    })
    .collect()
}

/// Standard base64 of `data` (for a `data:` URL). Small and self-contained so
/// the PWA needn't pull an encoder crate.
fn base64(data: &[u8]) -> String {
  const A: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
  for chunk in data.chunks(3) {
    let b0 = chunk[0] as u32;
    let b1 = *chunk.get(1).unwrap_or(&0) as u32;
    let b2 = *chunk.get(2).unwrap_or(&0) as u32;
    let n = (b0 << 16) | (b1 << 8) | b2;
    out.push(A[(n >> 18 & 63) as usize] as char);
    out.push(A[(n >> 12 & 63) as usize] as char);
    out.push(if chunk.len() > 1 {
      A[(n >> 6 & 63) as usize] as char
    } else {
      '='
    });
    out.push(if chunk.len() > 2 { A[(n & 63) as usize] as char } else { '=' });
  }
  out
}

#[cfg(test)]
mod tests {
  use super::base64;

  #[test]
  fn base64_matches_known_vectors() {
    assert_eq!(base64(b""), "");
    assert_eq!(base64(b"f"), "Zg==");
    assert_eq!(base64(b"fo"), "Zm8=");
    assert_eq!(base64(b"foo"), "Zm9v");
    assert_eq!(base64(b"foob"), "Zm9vYg==");
    assert_eq!(base64(b"foobar"), "Zm9vYmFy");
  }
}
