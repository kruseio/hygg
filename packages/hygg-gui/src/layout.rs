//! Responsive sizing: pick a font size so a `col`-character monospace line
//! fills the viewport width (then centers), capped on very wide windows so
//! desktop lines stay a comfortable length — the same fit the PWA does with DOM
//! measurement, here from the known advance width of a monospace glyph.

/// The reader's monospace font. We bundle **Fira Mono** (SIL OFL) and register
/// it at startup with `.font(MONO_FONT)` so the reader renders identically
/// regardless of the system font stack. It also pairs with the bundled Fira
/// Sans used for the UI chrome.
///
/// The bundled face is the *Medium* weight, and iced/glyphon renders nothing
/// when the requested weight has no matching face — so the weight is pinned to
/// `Medium` here rather than the `Weight::Normal` that `Font::with_name`
/// implies.
pub const MONO: iced::Font = iced::Font {
  family: iced::font::Family::Name("Fira Mono"),
  weight: iced::font::Weight::Medium,
  stretch: iced::font::Stretch::Normal,
  style: iced::font::Style::Normal,
};

/// Cap the rendered line width on large windows (keeps desktop readable).
const MAX_LINE_PX: f64 = 880.0;
/// Fraction of the container width the column occupies (rest is side margin).
const FILL: f64 = 0.96;
/// Advance width of one monospace glyph as a fraction of the font size. iced's
/// default monospace (Fira Mono) sits at ~0.6 em; measured empirically so a
/// `col`-char line lands within a couple of pixels of the container width.
const CHAR_ADVANCE: f64 = 0.6;

/// Font size (px) so a `col`-char line fills the container width (capped),
/// scaled by `zoom`, clamped to a sane range.
pub fn fit_font_px(container_w: f64, col: usize, zoom: f64) -> f64 {
  if col == 0 || container_w <= 1.0 {
    return 18.0 * zoom;
  }
  let target = (container_w * FILL).min(MAX_LINE_PX);
  let base = target / (col as f64 * CHAR_ADVANCE);
  (base * zoom).clamp(9.0, 34.0)
}

/// Line height in px — 1.0 line-height like the PWA so ASCII-art half-blocks
/// stack seamlessly, matching the terminal.
pub fn line_height(font_px: f64) -> f64 {
  font_px.round().max(1.0)
}

/// Advance width of one monospace glyph at `font_px`. The reader is monospace,
/// so this is the horizontal step between columns — used to map a pointer to a
/// column and to place the selection highlight.
pub fn char_advance(font_px: f64) -> f64 {
  font_px * CHAR_ADVANCE
}

/// Rendered pixel width of the reading column: a `col`-char line at `font`,
/// capped at the viewport. `col == 0` (server text of unknown width) fills the
/// viewport. The reader centers this block and `select` inverts the same
/// margin, so both must agree — hence one shared definition.
pub fn block_width(col: usize, font: f64, viewport: f32) -> f32 {
  if col == 0 {
    viewport
  } else {
    (col as f32 * char_advance(font) as f32).min(viewport)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn font_stays_within_clamp() {
    for w in [1.0, 320.0, 1440.0, 6000.0] {
      let px = fit_font_px(w, 64, 1.0);
      assert!((9.0..=34.0).contains(&px), "px {px} out of range at w={w}");
    }
  }

  #[test]
  fn wider_window_never_shrinks_font() {
    let narrow = fit_font_px(400.0, 64, 1.0);
    let wide = fit_font_px(1000.0, 64, 1.0);
    assert!(wide >= narrow);
  }

  #[test]
  fn zoom_scales_until_clamped() {
    // At a mid width the base sits below the cap, so zoom scales it up.
    let base = fit_font_px(500.0, 80, 1.0);
    let zoomed = fit_font_px(500.0, 80, 1.3);
    assert!(zoomed > base);
  }

  #[test]
  fn degenerate_inputs_are_safe() {
    assert!(fit_font_px(0.0, 0, 1.0) > 0.0);
    assert!(line_height(0.0) >= 1.0);
  }

  #[test]
  fn block_width_caps_and_fills_on_unknown_col() {
    let adv = char_advance(20.0) as f32; // 12 px
    // A known column is `col * adv`, capped at the viewport.
    assert_eq!(block_width(10, 20.0, 500.0), 10.0 * adv);
    assert_eq!(block_width(100, 20.0, 500.0), 500.0); // capped
    // Server text of unknown width (col == 0) fills the viewport, so the
    // reader's centering margin collapses to zero rather than half the width.
    assert_eq!(block_width(0, 20.0, 500.0), 500.0);
  }
}
