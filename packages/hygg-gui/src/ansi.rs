//! Convert a raw-ANSI art row (truecolor `▀` half-blocks emitted by
//! `cli-image-to-ascii` / the PDF image extractor) into iced rich-text spans so
//! the reader shows the same colored ASCII art the terminal does.
//!
//! Supports the SGR codes the renderer actually produces: truecolor fg
//! (`38;2;r;g;b`), truecolor bg (`48;2;r;g;b`), and resets (`0` / `39` / `49`).

use iced::widget::text::Span;
use iced::{Color, Font};

#[derive(Clone, Copy, Default, PartialEq)]
struct Style {
  fg: Option<(u8, u8, u8)>,
  bg: Option<(u8, u8, u8)>,
}

/// Parse an ANSI line into styled spans. `default_fg` colors runs with no
/// explicit foreground. Each span carries the monospace font so columns align.
pub fn ansi_to_spans<'a>(
  line: &str,
  default_fg: Color,
) -> Vec<Span<'a, (), Font>> {
  let mut spans = Vec::new();
  let mut style = Style::default();
  let mut pending = String::new();
  let bytes = line.as_bytes();
  let mut i = 0;

  while i < bytes.len() {
    if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
      flush(&mut spans, &mut pending, style, default_fg);
      let mut j = i + 2;
      while j < bytes.len() && bytes[j] != b'm' {
        j += 1;
      }
      let params = std::str::from_utf8(&bytes[i + 2..j]).unwrap_or("");
      apply(&mut style, params);
      i = if j < bytes.len() { j + 1 } else { j };
    } else {
      let ch_len = utf8_len(bytes[i]);
      let end = (i + ch_len).min(bytes.len());
      pending.push_str(std::str::from_utf8(&bytes[i..end]).unwrap_or(""));
      i = end;
    }
  }
  flush(&mut spans, &mut pending, style, default_fg);
  spans
}

fn flush<'a>(
  spans: &mut Vec<Span<'a, (), Font>>,
  pending: &mut String,
  style: Style,
  default_fg: Color,
) {
  if pending.is_empty() {
    return;
  }
  let fg = style.fg.map(|(r, g, b)| rgb(r, g, b)).unwrap_or(default_fg);
  let mut span =
    Span::new(std::mem::take(pending)).font(crate::layout::MONO).color(fg);
  if let Some((r, g, b)) = style.bg {
    span = span.background(rgb(r, g, b));
  }
  spans.push(span);
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
  Color::from_rgb8(r, g, b)
}

fn apply(style: &mut Style, params: &str) {
  let toks: Vec<u16> =
    params.split(';').filter_map(|t| t.parse().ok()).collect();
  if toks.is_empty() {
    *style = Style::default();
    return;
  }
  let mut k = 0;
  while k < toks.len() {
    match toks[k] {
      0 => *style = Style::default(),
      39 => style.fg = None,
      49 => style.bg = None,
      38 | 48 if toks.get(k + 1) == Some(&2) && k + 4 < toks.len() => {
        let rgb = (toks[k + 2] as u8, toks[k + 3] as u8, toks[k + 4] as u8);
        if toks[k] == 38 {
          style.fg = Some(rgb);
        } else {
          style.bg = Some(rgb);
        }
        k += 4;
      }
      _ => {}
    }
    k += 1;
  }
}

fn utf8_len(first: u8) -> usize {
  match first {
    b if b < 0x80 => 1,
    b if b >> 5 == 0b110 => 2,
    b if b >> 4 == 0b1110 => 3,
    b if b >> 3 == 0b11110 => 4,
    _ => 1,
  }
}
