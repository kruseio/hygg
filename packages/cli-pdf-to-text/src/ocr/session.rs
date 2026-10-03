//! OCR of a single raster image for callers outside this crate: a GUI that
//! renders a page itself and wants the recognized lines with their
//! positions (hygg office's *Recognize text*). Same bundled models and
//! configuration as the extraction path above.

/// One recognized line: its text, its quadrilateral in image pixels
/// (top-left, top-right, bottom-right, bottom-left) and the recognizer's
/// confidence (0–1).
#[derive(Clone, Debug)]
pub struct OcrLine {
  pub text: String,
  pub polygon: [[f32; 2]; 4],
  pub confidence: f32,
}

/// A loaded OCR engine (`Send + Sync`; loading takes a moment, so keep it).
pub struct OcrSession(pdf_oxide::ocr::OcrEngine);

impl OcrSession {
  /// Loads the engine, downloading and verifying the models first if they
  /// aren't cached yet (see [`ocr_models_cached`]). Text is detected on
  /// the image scaled to at most `det_max_side` pixels: 960 suits
  /// embedded images, a whole rendered page needs about 1600–2000.
  pub fn new(det_max_side: u32) -> Result<Self, String> {
    super::bundled_ocr_engine_with(det_max_side)
      .map(Self)
      .map_err(|e| e.to_string())
  }

  /// The lines recognized in `image`. Images over 4000 px are scaled down
  /// first; the polygons are in the coordinates of the image passed in.
  pub fn recognize(
    &self,
    image: &image::DynamicImage,
  ) -> Result<Vec<OcrLine>, String> {
    let guarded = super::ocr_size_guarded(image);
    let k = if guarded.width() == image.width() {
      1.0
    } else {
      image.width() as f32 / guarded.width() as f32
    };
    let out = self.0.ocr_image(&guarded).map_err(|e| e.to_string())?;
    Ok(
      out
        .spans
        .into_iter()
        .map(|s| OcrLine {
          text: s.text,
          polygon: s.polygon.map(|[x, y]| [x * k, y * k]),
          confidence: s.confidence,
        })
        .collect(),
    )
  }
}

/// Whether the OCR models are already on disk (no download needed).
pub fn ocr_models_cached() -> bool {
  super::files::models_cached()
}

/// How many bytes the first use downloads.
pub const OCR_MODELS_DOWNLOAD_BYTES: u64 = super::files::DOWNLOAD_BYTES;
