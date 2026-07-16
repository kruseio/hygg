#[cfg(feature = "ocr")]
use pdf_oxide::geometry::Rect;

#[cfg(feature = "ocr")]
#[derive(Clone, Debug)]
pub(crate) struct TextRegion {
  pub(crate) left: f32,
  pub(crate) bottom: f32,
  pub(crate) right: f32,
  pub(crate) top: f32,
}

#[cfg(feature = "ocr")]
impl TextRegion {
  pub(crate) fn from_rect(rect: &Rect) -> Option<Self> {
    let left = rect.left();
    let right = rect.right();
    let bottom = rect.top();
    let top = rect.bottom();
    if !left.is_finite()
      || !right.is_finite()
      || !bottom.is_finite()
      || !top.is_finite()
      || right <= left
      || top <= bottom
    {
      return None;
    }
    Some(Self { left, bottom, right, top })
  }

  pub(crate) fn width(&self) -> f32 {
    self.right - self.left
  }

  pub(crate) fn height(&self) -> f32 {
    self.top - self.bottom
  }

  pub(crate) fn overlaps_or_near(&self, other: &Self) -> bool {
    let pad_x = self.width().max(other.width()).max(12.0) * 0.25;
    let pad_y = self.height().max(other.height()).max(12.0) * 0.75;
    self.left <= other.right + pad_x
      && self.right + pad_x >= other.left
      && self.bottom <= other.top + pad_y
      && self.top + pad_y >= other.bottom
  }
}

#[cfg(feature = "ocr")]
#[derive(Clone, Debug)]
pub(crate) struct PositionedText {
  pub(crate) text: String,
  pub(crate) region: TextRegion,
  pub(crate) confidence: f32,
}
