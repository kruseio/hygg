#[cfg(feature = "pdf-ocr-bundled")]
use super::region::PositionedText;

#[cfg(feature = "pdf-ocr-bundled")]
pub(crate) fn dedupe_positioned_ocr(
  ocr_regions: Vec<PositionedText>,
) -> Vec<PositionedText> {
  let mut out: Vec<PositionedText> = Vec::new();
  for region in ocr_regions {
    let normalized = normalized_text(&region.text);
    if normalized.is_empty() {
      continue;
    }
    let mut duplicate_index = None;
    for (idx, existing) in out.iter().enumerate() {
      let existing_normalized = normalized_text(&existing.text);
      if existing.region.overlaps_or_near(&region.region)
        && (existing_normalized.contains(&normalized)
          || normalized.contains(&existing_normalized))
      {
        duplicate_index = Some(idx);
        break;
      }
    }
    if let Some(idx) = duplicate_index {
      if region.confidence > out[idx].confidence {
        out[idx] = region;
      }
      continue;
    }
    out.push(region);
  }
  out
}

#[cfg(feature = "pdf-ocr-bundled")]
pub(crate) fn merge_native_and_ocr_regions_text(
  native: &str,
  native_regions: &[PositionedText],
  ocr_regions: &[PositionedText],
) -> String {
  let native = native.trim();
  let mut extra = Vec::new();
  for ocr in ocr_regions {
    if is_native_duplicate(native_regions, ocr) {
      continue;
    }
    extra.push(ocr);
  }

  extra.sort_by(|a, b| {
    b.region
      .top
      .partial_cmp(&a.region.top)
      .unwrap_or(std::cmp::Ordering::Equal)
      .then_with(|| {
        a.region
          .left
          .partial_cmp(&b.region.left)
          .unwrap_or(std::cmp::Ordering::Equal)
      })
  });

  if extra.is_empty() {
    return native.to_string();
  }
  let ocr = extra
    .iter()
    .map(|region| region.text.trim())
    .filter(|text| !text.is_empty())
    .collect::<Vec<_>>()
    .join("\n");
  if native.is_empty() { ocr } else { format!("{native}\n{ocr}") }
}

#[cfg(feature = "pdf-ocr-bundled")]
fn is_native_duplicate(
  native_regions: &[PositionedText],
  ocr: &PositionedText,
) -> bool {
  let ocr_normalized = normalized_text(&ocr.text);
  if ocr_normalized.is_empty() {
    return true;
  }
  native_regions.iter().any(|native| {
    let native_normalized = normalized_text(&native.text);
    native.region.overlaps_or_near(&ocr.region)
      && (native_normalized.contains(&ocr_normalized)
        || ocr_normalized.contains(&native_normalized))
  })
}

#[cfg(feature = "pdf-ocr-bundled")]
pub(crate) fn normalized_text(text: &str) -> String {
  text
    .chars()
    .filter(|ch| ch.is_alphanumeric())
    .flat_map(char::to_lowercase)
    .collect()
}
