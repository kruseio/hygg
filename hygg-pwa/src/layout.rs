//! Responsive sizing: pick a font size so a `col`-character monospace line
//! fills the viewport width (then centers), capped on very wide screens so
//! desktop lines stay a comfortable length. Width is measured from the real
//! font so the fit is exact regardless of which monospace the device falls back
//! to.

use wasm_bindgen::JsCast;

/// Must match `--font-mono` in styles/main.css.
const FONT_STACK: &str =
  "ui-monospace, 'SF Mono', 'JetBrains Mono', Menlo, Consolas, monospace";
/// Cap the rendered line width on large screens (keeps desktop readable).
const MAX_LINE_PX: f64 = 880.0;
/// Fraction of the container width the column occupies (rest is side margin).
const FILL: f64 = 0.96;

/// Font size (px) so a `col`-char line fills the container width (capped),
/// scaled by `zoom`, clamped to a sane range.
pub fn fit_font_px(container_w: f64, col: usize, zoom: f64) -> f64 {
  if col == 0 || container_w <= 1.0 {
    return 18.0 * zoom;
  }
  let target = (container_w * FILL).min(MAX_LINE_PX);
  let w_at_100 =
    measure_line_px(col, 100.0).unwrap_or(col as f64 * 60.0).max(1.0);
  // Glyph width scales linearly with font size, so one measurement suffices.
  let base = target / w_at_100 * 100.0;
  (base * zoom).clamp(9.0, 34.0)
}

/// Width in px of a `col`-character run rendered at `font_px` in the reader
/// font, via an off-screen span (exact, font-aware).
fn measure_line_px(col: usize, font_px: f64) -> Option<f64> {
  let doc = web_sys::window()?.document()?;
  let span: web_sys::HtmlElement =
    doc.create_element("span").ok()?.dyn_into().ok()?;
  let style = span.style();
  let _ = style.set_property("position", "absolute");
  let _ = style.set_property("left", "-9999px");
  let _ = style.set_property("visibility", "hidden");
  let _ = style.set_property("white-space", "pre");
  let _ = style.set_property("font-family", FONT_STACK);
  let _ = style.set_property("font-size", &format!("{font_px}px"));
  span.set_text_content(Some(&"0".repeat(col)));
  let body = doc.body()?;
  body.append_child(&span).ok()?;
  let width = span.get_bounding_client_rect().width();
  let _ = body.remove_child(&span);
  Some(width)
}
